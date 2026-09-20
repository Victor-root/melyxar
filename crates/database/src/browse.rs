//! Reading the library the way a grid reads it.
//!
//! Everything here is built for one thing: a page of cards has to arrive fast
//! and at a steady cost, whatever the size of the collection and however deep
//! the viewer has scrolled.
//!
//! That is why paging works from a cursor rather than from an offset. Asking
//! for the thousandth page by offset makes the engine walk the nine hundred
//! and ninety nine before it, so a library gets slower the further you go;
//! asking for what comes after a known row is an index seek and costs the same
//! everywhere. It also survives a scan adding a film while someone reads,
//! which an offset does not: with an offset, one insertion shifts every page
//! and a card appears twice or not at all.

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::time::{Millis, Timestamp};
use melyxar_core::work::{IdentificationNote, IdentificationState, WorkKind};
use sqlx::{AssertSqlSafe, Row};

use crate::convert::{parse_id, parse_timestamp};
use crate::images::StoredImage;
use crate::{Database, DatabaseError, Result};

/// What a viewer meets on their own, out of everything a library holds.
///
/// A season is opened from its series and an episode from its season; neither
/// is ever met on its own in the middle of the films. So neither is shown in a
/// grid, counted in its total, offered as a letter, or counted in the menus
/// that narrow it.
///
/// An episode that belongs to no season is the exception, and it is met on its
/// own: a file whose name never said which episode it is has no season to be
/// opened from, so hiding it here is hiding it everywhere. It was scanned,
/// analysed and stored, and no page could reach it. A file that is merely
/// misnamed shows up in the report and gets corrected; a file that is nowhere
/// is never even looked for.
///
/// Written once because the five places that answer "what is in this library"
/// have to agree. Two of them had already been written without it: the genre
/// menu and the decade menu counted seasons and episodes, so the day a series
/// is in the collection they would have offered "Drama, forty eight" over a
/// grid holding four. Which is precisely the fault the genre menu's own
/// comment says it exists to avoid.
///
/// Takes the table it is written about, because half these queries join and
/// half do not, and `kind` alone is ambiguous as soon as something else in the
/// statement carries one.
fn met_on_its_own(table: &str) -> String {
    format!(
        "({table}kind IN ('movie', 'series', 'album')
          OR ({table}kind = 'episode' AND {table}parent_id IS NULL))"
    )
}

/// What a grid can be ordered by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkOrder {
    /// Alphabetical, ignoring a leading article and accents.
    Title,
    /// Newest in the library first, which is what a home page shows.
    AddedAt,
    ReleaseYear,
    CommunityRating,
    Runtime,
}

impl WorkOrder {
    /// The column ordered on, and the one that breaks a tie.
    ///
    /// Written out rather than assembled from anything a caller sends, so no
    /// part of a statement here can come from outside.
    const fn statements(self) -> OrderStatements {
        match self {
            Self::Title => OrderStatements {
                column: "w.sort_title",
                ascending_order: "ORDER BY w.sort_title, w.id",
                descending_order: "ORDER BY w.sort_title DESC, w.id DESC",
                ascending_after: "(w.sort_title, w.id) > (SELECT sort_title, id FROM works WHERE id = ?)",
                descending_after: "(w.sort_title, w.id) < (SELECT sort_title, id FROM works WHERE id = ?)",
            },
            Self::AddedAt => OrderStatements {
                column: "w.added_at",
                ascending_order: "ORDER BY w.added_at, w.id",
                descending_order: "ORDER BY w.added_at DESC, w.id DESC",
                ascending_after: "(w.added_at, w.id) > (SELECT added_at, id FROM works WHERE id = ?)",
                descending_after: "(w.added_at, w.id) < (SELECT added_at, id FROM works WHERE id = ?)",
            },
            Self::ReleaseYear => OrderStatements {
                column: "w.release_year",
                ascending_order: "ORDER BY w.release_year, w.id",
                descending_order: "ORDER BY w.release_year DESC, w.id DESC",
                ascending_after: "(w.release_year, w.id) > (SELECT release_year, id FROM works WHERE id = ?)",
                descending_after: "(w.release_year, w.id) < (SELECT release_year, id FROM works WHERE id = ?)",
            },
            Self::CommunityRating => OrderStatements {
                column: "w.community_rating",
                ascending_order: "ORDER BY w.community_rating, w.id",
                descending_order: "ORDER BY w.community_rating DESC, w.id DESC",
                ascending_after: "(w.community_rating, w.id) > (SELECT community_rating, id FROM works WHERE id = ?)",
                descending_after: "(w.community_rating, w.id) < (SELECT community_rating, id FROM works WHERE id = ?)",
            },
            Self::Runtime => OrderStatements {
                column: "w.runtime_ms",
                ascending_order: "ORDER BY w.runtime_ms, w.id",
                descending_order: "ORDER BY w.runtime_ms DESC, w.id DESC",
                ascending_after: "(w.runtime_ms, w.id) > (SELECT runtime_ms, id FROM works WHERE id = ?)",
                descending_after: "(w.runtime_ms, w.id) < (SELECT runtime_ms, id FROM works WHERE id = ?)",
            },
        }
    }
}

struct OrderStatements {
    /// Only used to keep rows with nothing to order on out of the way.
    column: &'static str,
    ascending_order: &'static str,
    descending_order: &'static str,
    ascending_after: &'static str,
    descending_after: &'static str,
}

/// What a grid is being asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowseRequest {
    pub library_id: Option<LibraryId>,
    /// The letter a grid was asked to start at.
    pub initial: Option<Initial>,
    pub order: WorkOrder,
    pub descending: bool,
    /// The last card of the previous page. Absent for the first page.
    pub after: Option<WorkId>,
    pub limit: i64,
    pub genre: Option<String>,
    /// Films of one decade, given as its first year.
    pub decade: Option<i32>,
    /// Words to look for in the title.
    pub search: Option<String>,
    /// Only the works nobody has managed to identify.
    pub unidentified_only: bool,
}

/// The letter a title begins with, as a grid is asked to jump to it.
///
/// A collection of a few hundred films is too long to scroll through and too
/// short to search by hand every time. What is wanted is the letter, which is
/// also the only thing a viewer reliably remembers about a title they are
/// looking for.
///
/// Everything that begins with no letter at all shares one bucket: digits,
/// symbols, and the alphabets the fold to plain letters does not reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Initial {
    Letter(char),
    Other,
}

/// What the bucket for everything else is written as, in an address and on a
/// button alike.
const OTHER_INITIAL: &str = "#";

impl Initial {
    /// Reads one from what a client sent, refusing anything else.
    pub fn parse(value: &str) -> Option<Self> {
        if value == OTHER_INITIAL {
            return Some(Self::Other);
        }
        let mut characters = value.chars();
        match (characters.next(), characters.next()) {
            (Some(letter), None) if letter.is_ascii_alphabetic() => {
                Some(Self::Letter(letter.to_ascii_lowercase()))
            }
            _ => None,
        }
    }

    /// How it is written back to a client.
    pub fn as_text(self) -> String {
        match self {
            Self::Letter(letter) => letter.to_string(),
            Self::Other => OTHER_INITIAL.to_string(),
        }
    }

    /// The half open range of ordering titles this letter covers.
    ///
    /// A range rather than a test on the first character, so the ordering
    /// index does the work: on a hundred thousand films the difference is a
    /// walk of the whole table against a seek.
    fn range(self) -> Option<(String, String)> {
        match self {
            Self::Letter(letter) => {
                let next = char::from(letter as u8 + 1);
                Some((letter.to_string(), next.to_string()))
            }
            Self::Other => None,
        }
    }
}

impl Default for BrowseRequest {
    fn default() -> Self {
        Self {
            library_id: None,
            initial: None,
            order: WorkOrder::Title,
            descending: false,
            after: None,
            limit: DEFAULT_PAGE,
            genre: None,
            decade: None,
            search: None,
            unidentified_only: false,
        }
    }
}

/// How many cards a page holds when nobody says otherwise.
pub const DEFAULT_PAGE: i64 = 60;
/// The most a single page may hold, so one request cannot ask for everything.
pub const LARGEST_PAGE: i64 = 200;

/// One card in a grid.
///
/// Deliberately small: a grid shows a picture, a title and a year, and sending
/// a synopsis per card would multiply the weight of a page by ten for text
/// nobody reads there.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkCard {
    pub id: WorkId,
    pub library_id: LibraryId,
    pub kind: WorkKind,
    pub title: String,
    pub release_year: Option<i32>,
    pub runtime: Option<Millis>,
    pub community_rating: Option<f64>,
    pub identification: IdentificationState,
    /// What stopped the last look up, so a grid says why a film is nameless
    /// instead of only saying that it is.
    pub identification_note: Option<IdentificationNote>,
    /// Shown while the picture loads, so a grid has colour from the first
    /// moment instead of grey holes.
    pub dominant_color: Option<String>,
    pub added_at: Timestamp,
    /// Every size of the poster, largest first.
    pub poster: Vec<StoredImage>,
}

/// A page of cards, and how to ask for the next one.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkPage {
    pub cards: Vec<WorkCard>,
    /// Absent when this page is the last.
    pub next: Option<WorkId>,
}

impl Database {
    /// Reads one page of cards.
    pub async fn browse_works(&self, request: &BrowseRequest) -> Result<WorkPage> {
        let limit = request.limit.clamp(1, LARGEST_PAGE);
        let statements = request.order.statements();

        let mut sql = String::from(
            "SELECT DISTINCT w.id, w.library_id, w.kind, w.title, w.release_year, w.runtime_ms,
                    w.community_rating, w.identification, w.identification_note, w.dominant_color,
                    w.added_at
             FROM works w",
        );
        if request.genre.is_some() {
            sql.push_str(
                " JOIN work_genres wg ON wg.work_id = w.id
                  JOIN genres g ON g.id = wg.genre_id",
            );
        }
        sql.push_str(&format!(" WHERE {}", met_on_its_own("w.")));

        if request.library_id.is_some() {
            sql.push_str(" AND w.library_id = ?");
        }
        if request.genre.is_some() {
            sql.push_str(" AND g.name = ? COLLATE NOCASE");
        }
        if request.decade.is_some() {
            // The decade itself rather than the stretch of years it stands
            // for: one value is what an index can be walked into, and walked
            // into already in title order. A stretch would be read whole and
            // sorted before the first card could go out.
            sql.push_str(" AND w.decade = ?");
        }
        if request.search.is_some() {
            sql.push_str(" AND w.sort_title LIKE ? ESCAPE '\\'");
        }
        if request.unidentified_only {
            sql.push_str(" AND w.identification IN ('pending', 'unidentified')");
        }
        match request.initial {
            Some(Initial::Letter(_)) => {
                sql.push_str(" AND w.sort_title >= ? AND w.sort_title < ?");
            }
            // Everything the letters do not cover, which is everything sorting
            // before the first of them or after the last.
            Some(Initial::Other) => {
                sql.push_str(" AND (w.sort_title < 'a' OR w.sort_title >= '{')");
            }
            None => {}
        }
        // A film with nothing to order on would otherwise sit at one end of
        // every ordering and be the first thing anyone sees.
        if !matches!(request.order, WorkOrder::Title | WorkOrder::AddedAt) {
            sql.push_str(" AND ");
            sql.push_str(statements.column);
            sql.push_str(" IS NOT NULL");
        }
        if request.after.is_some() {
            sql.push_str(" AND ");
            sql.push_str(match request.descending {
                true => statements.descending_after,
                false => statements.ascending_after,
            });
        }

        sql.push(' ');
        sql.push_str(match request.descending {
            true => statements.descending_order,
            false => statements.ascending_order,
        });
        // One more than asked for, which is how the answer knows whether
        // another page exists without counting the whole library.
        sql.push_str(" LIMIT ?");

        // Every piece of the statement above comes from a constant in this
        // file, chosen by a match on values of our own types. Nothing a caller
        // sends is ever put in the text: names and words go in as bound
        // values, which is what the wrapper is being told here.
        let mut query = sqlx::query(AssertSqlSafe(sql));
        if let Some(library_id) = request.library_id {
            query = query.bind(library_id.to_db_string());
        }
        if let Some(genre) = &request.genre {
            query = query.bind(genre.clone());
        }
        if let Some(decade) = request.decade {
            query = query.bind(decade);
        }
        if let Some(search) = &request.search {
            query = query.bind(format!("%{}%", escape_for_like(search)));
        }
        if let Some((from, to)) = request.initial.and_then(Initial::range) {
            query = query.bind(from).bind(to);
        }
        if let Some(after) = request.after {
            query = query.bind(after.to_db_string());
        }
        query = query.bind(limit + 1);

        let rows = query.fetch_all(self.reader()).await?;
        let mut cards: Vec<WorkCard> =
            rows.iter().map(card_from_row).collect::<Result<Vec<_>>>()?;

        let next = match cards.len() as i64 > limit {
            true => {
                cards.truncate(limit as usize);
                cards.last().map(|card| card.id)
            }
            false => None,
        };

        self.attach_posters(&mut cards).await?;
        Ok(WorkPage { cards, next })
    }

    /// Puts the poster of every card in place.
    ///
    /// One query for the whole page rather than one per card: a grid of sixty
    /// cards would otherwise cost sixty round trips to show one screen.
    pub(crate) async fn attach_posters(&self, cards: &mut [WorkCard]) -> Result<()> {
        if cards.is_empty() {
            return Ok(());
        }

        let owners: Vec<String> = cards.iter().map(|card| card.id.to_db_string()).collect();
        // The list is built from identifiers this crate just read back, never
        // from anything a caller sent.
        let placeholders = vec!["?"; owners.len()].join(", ");
        let sql = format!(
            "SELECT {} FROM images
             WHERE owner_kind = 'work' AND image_kind = 'poster' AND owner_id IN ({placeholders})
             ORDER BY width DESC",
            crate::images::WHAT_A_PICTURE_IS
        );

        // The only thing assembled here is a row of question marks, one per
        // identifier this crate has just read back from its own tables.
        let mut query = sqlx::query(AssertSqlSafe(sql));
        for owner in &owners {
            query = query.bind(owner);
        }
        let rows = query.fetch_all(self.reader()).await?;

        for row in rows {
            let owner_id: String = row.try_get("owner_id")?;
            let image = crate::images::image_from_row(&row)?;
            if let Some(card) = cards
                .iter_mut()
                .find(|card| card.id.to_db_string() == owner_id)
            {
                card.poster.push(image);
            }
        }
        Ok(())
    }

    /// How many works a grid would show for this request.
    ///
    /// Asked for separately and only when a total is actually displayed: a
    /// count walks the whole set, which is exactly what paging avoids.
    pub async fn count_browsable(&self, library_id: Option<LibraryId>) -> Result<i64> {
        let row: (i64,) = match library_id {
            Some(id) => {
                sqlx::query_as(AssertSqlSafe(format!(
                    "SELECT count(*) FROM works WHERE library_id = ? AND {}",
                    met_on_its_own("")
                )))
                .bind(id.to_db_string())
                .fetch_one(self.reader())
                .await?
            }
            None => {
                sqlx::query_as(AssertSqlSafe(format!(
                    "SELECT count(*) FROM works WHERE {}",
                    met_on_its_own("")
                )))
                .fetch_one(self.reader())
                .await?
            }
        };
        Ok(row.0)
    }

    /// How many works are still waiting for a name.
    ///
    /// Counted over everything a library holds rather than over what a grid
    /// shows: an episode nobody could name is a film nobody could name, and
    /// the list that answers for it reaches both.
    pub async fn count_awaiting_identification(
        &self,
        library_id: Option<LibraryId>,
    ) -> Result<i64> {
        let row: (i64,) = match library_id {
            Some(id) => sqlx::query_as(
                "SELECT count(*) FROM works
                  WHERE library_id = ? AND identification IN ('pending', 'unidentified')",
            )
            .bind(id.to_db_string())
            .fetch_one(self.reader())
            .await?,
            None => sqlx::query_as(
                "SELECT count(*) FROM works
                  WHERE identification IN ('pending', 'unidentified')",
            )
            .fetch_one(self.reader())
            .await?,
        };
        Ok(row.0)
    }

    /// Every genre that is actually in use, with how many works carry it.
    ///
    /// What a filter menu is built from: offering a genre nobody has leads to
    /// an empty grid and looks like a fault.
    pub async fn genres_in_use(&self, library_id: Option<LibraryId>) -> Result<Vec<(String, i64)>> {
        let rows = match library_id {
            Some(id) => {
                sqlx::query(AssertSqlSafe(format!(
                    "SELECT g.name, count(*) AS total FROM genres g
                 JOIN work_genres wg ON wg.genre_id = g.id
                 JOIN works w ON w.id = wg.work_id
                 WHERE w.library_id = ? AND {}
                 GROUP BY g.id ORDER BY total DESC, g.name",
                    met_on_its_own("w.")
                )))
                .bind(id.to_db_string())
                .fetch_all(self.reader())
                .await?
            }
            None => {
                sqlx::query(AssertSqlSafe(format!(
                    "SELECT g.name, count(*) AS total FROM genres g
                 JOIN work_genres wg ON wg.genre_id = g.id
                 JOIN works w ON w.id = wg.work_id
                 WHERE {}
                 GROUP BY g.id ORDER BY total DESC, g.name",
                    met_on_its_own("w.")
                )))
                .fetch_all(self.reader())
                .await?
            }
        };

        rows.iter()
            .map(|row| Ok((row.try_get("name")?, row.try_get("total")?)))
            .collect()
    }

    /// Every letter a title starts with, with how many start with it.
    ///
    /// What the row of letters is built from. Offering a letter nobody has
    /// leads to an empty grid and looks like a fault, exactly as it would for
    /// a genre.
    pub async fn initials_in_use(
        &self,
        library_id: Option<LibraryId>,
    ) -> Result<Vec<(String, i64)>> {
        // The bucket for everything beginning with no letter sorts before the
        // letters, which is where a row of them wants it.
        let counted = "SELECT CASE
                           WHEN substr(sort_title, 1, 1) BETWEEN 'a' AND 'z'
                           THEN substr(sort_title, 1, 1)
                           ELSE '#'
                         END AS initial,
                         count(*) AS total
                       FROM works
                       WHERE ";
        let counted = format!("{counted}{}", met_on_its_own(""));

        let rows = match library_id {
            Some(id) => {
                sqlx::query(AssertSqlSafe(format!(
                    "{counted} AND library_id = ? GROUP BY initial ORDER BY initial"
                )))
                .bind(id.to_db_string())
                .fetch_all(self.reader())
                .await?
            }
            None => {
                sqlx::query(AssertSqlSafe(format!(
                    "{counted} GROUP BY initial ORDER BY initial"
                )))
                .fetch_all(self.reader())
                .await?
            }
        };

        rows.iter()
            .map(|row| Ok((row.try_get("initial")?, row.try_get("total")?)))
            .collect()
    }

    /// The decades the collection actually spans, newest first.
    pub async fn decades_in_use(&self, library_id: Option<LibraryId>) -> Result<Vec<(i32, i64)>> {
        let rows = match library_id {
            Some(id) => {
                sqlx::query(AssertSqlSafe(format!(
                    "SELECT decade, count(*) AS total FROM works
                 WHERE decade IS NOT NULL AND library_id = ? AND {}
                 GROUP BY decade ORDER BY decade DESC",
                    met_on_its_own("")
                )))
                .bind(id.to_db_string())
                .fetch_all(self.reader())
                .await?
            }
            None => {
                sqlx::query(AssertSqlSafe(format!(
                    "SELECT decade, count(*) AS total FROM works
                 WHERE decade IS NOT NULL AND {}
                 GROUP BY decade ORDER BY decade DESC",
                    met_on_its_own("")
                )))
                .fetch_all(self.reader())
                .await?
            }
        };

        rows.iter()
            .map(|row| Ok((row.try_get("decade")?, row.try_get("total")?)))
            .collect()
    }
}

/// Makes the characters a pattern gives a meaning to stand for themselves.
///
/// Without this, a film whose title holds a percent sign matches everything.
fn escape_for_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub(crate) fn card_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<WorkCard> {
    let kind_text: String = row.try_get("kind")?;
    let identification_text: String = row.try_get("identification")?;

    Ok(WorkCard {
        id: parse_id(&row.try_get::<String, _>("id")?)?,
        library_id: parse_id(&row.try_get::<String, _>("library_id")?)?,
        kind: WorkKind::parse(&kind_text)
            .ok_or_else(|| DatabaseError::Corrupt(format!("work kind '{kind_text}' is unknown")))?,
        title: row.try_get("title")?,
        release_year: row.try_get("release_year")?,
        runtime: row
            .try_get::<Option<i64>, _>("runtime_ms")?
            .map(Millis::new),
        community_rating: row.try_get("community_rating")?,
        identification: IdentificationState::parse(&identification_text).ok_or_else(|| {
            DatabaseError::Corrupt(format!(
                "identification state '{identification_text}' is unknown"
            ))
        })?,
        identification_note: crate::catalogue::identification_note_from_row(row)?,
        dominant_color: row.try_get("dominant_color")?,
        added_at: parse_timestamp(&row.try_get::<String, _>("added_at")?)?,
        poster: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::library::LibraryKind;
    use std::path::PathBuf;

    /// A library holding the given films, as (title, year, rating).
    async fn library_of(films: &[(&str, i32, f64)]) -> (Database, LibraryId) {
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

        for (title, year, rating) in films {
            let work = database
                .create_work(
                    library.id,
                    WorkKind::Movie,
                    title,
                    &melyxar_library_sort_title(title),
                    Some(*year),
                )
                .await
                .expect("work created");
            sqlx::query(
                "UPDATE works SET community_rating = ?, runtime_ms = ?, identification = 'identified'
                 WHERE id = ?",
            )
            .bind(rating)
            .bind(*year as i64 * 1000)
            .bind(work.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("film completed");
        }
        (database, library.id)
    }

    #[tokio::test]
    async fn the_works_still_waiting_for_a_name_are_counted_where_they_are() {
        // Over everything a library holds rather than over what a grid shows:
        // an episode nobody could name is a film nobody could name.
        let (database, library_id) = library_of(&[("Quiet Harbour", 2019, 7.4)]).await;
        assert_eq!(
            database
                .count_awaiting_identification(Some(library_id))
                .await
                .expect("counted"),
            0,
            "the films of this library all have names"
        );

        let nameless = database
            .create_work(
                library_id,
                WorkKind::Movie,
                "Untitled 1994",
                "untitled 1994",
                None,
            )
            .await
            .expect("work created");
        assert_eq!(
            database
                .count_awaiting_identification(Some(library_id))
                .await
                .expect("counted"),
            1
        );
        assert_eq!(
            database
                .count_awaiting_identification(None)
                .await
                .expect("counted"),
            1,
            "asked of the whole catalogue it answers the same here"
        );

        database
            .mark_work_unidentified(nameless.id)
            .await
            .expect("marked");
        assert_eq!(
            database
                .count_awaiting_identification(Some(library_id))
                .await
                .expect("counted"),
            1,
            "a film nothing could name is still a film waiting for one"
        );
    }

    #[tokio::test]
    async fn every_way_of_counting_a_library_counts_the_same_works() {
        // A season is opened from its series and an episode from its season:
        // neither is ever met on its own in a grid. Five places answer "what
        // is in this library" and they have to agree. Two of them did not: the
        // genre menu and the decade menu counted seasons and episodes, so a
        // collection holding one series would have offered "Drame, three" over
        // a grid holding one card.
        let (database, library_id) = library_of(&[("Quiet Harbour", 2019, 7.4)]).await;

        // The one film, plus a series with a season and an episode under it,
        // all of the same decade and all carrying the same genre.
        let mut every = vec![
            database
                .browse_works(&BrowseRequest {
                    library_id: Some(library_id),
                    ..Default::default()
                })
                .await
                .expect("read")
                .cards[0]
                .id,
        ];
        for (kind, title) in [
            (WorkKind::Series, "Distant Signal"),
            (WorkKind::Season, "Distant Signal, first year"),
            (WorkKind::Episode, "Distant Signal, first night"),
        ] {
            let work = database
                .create_work(library_id, kind, title, &title.to_lowercase(), Some(2019))
                .await
                .expect("work created");
            every.push(work.id);
        }

        // Hung together the way a scan will hang them: the season under its
        // series, the episode under its season. An episode under nothing at
        // all is a different case entirely and is met on its own, which the
        // test below is about.
        for (child, parent) in [(2, 1), (3, 2)] {
            sqlx::query("UPDATE works SET parent_id = ? WHERE id = ?")
                .bind(every[parent].to_db_string())
                .bind(every[child].to_db_string())
                .execute(database.writer())
                .await
                .expect("hung under its parent");
        }

        sqlx::query("INSERT INTO genres (id, name) VALUES ('g1', 'Drame')")
            .execute(database.writer())
            .await
            .expect("genre created");
        for work in &every {
            sqlx::query("INSERT INTO work_genres (work_id, genre_id) VALUES (?, 'g1')")
                .bind(work.to_db_string())
                .execute(database.writer())
                .await
                .expect("genre attached");
        }

        // What the grid really shows: the film and the series, not the season
        // and not the episode.
        let shown = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                ..Default::default()
            })
            .await
            .expect("read")
            .cards
            .len() as i64;
        assert_eq!(shown, 2);

        assert_eq!(
            database
                .count_browsable(Some(library_id))
                .await
                .expect("read"),
            shown,
            "the total says what the grid shows"
        );
        assert_eq!(
            database
                .genres_in_use(Some(library_id))
                .await
                .expect("read"),
            vec![("Drame".to_string(), shown)],
            "and so does the genre menu, which used to count all four"
        );
        assert_eq!(
            database
                .decades_in_use(Some(library_id))
                .await
                .expect("read"),
            vec![(2010, shown)],
            "and the decade menu, which used to count all four as well"
        );
        assert_eq!(
            database
                .initials_in_use(Some(library_id))
                .await
                .expect("read")
                .iter()
                .map(|(_, total)| total)
                .sum::<i64>(),
            shown,
            "and the letters beside the grid"
        );
    }

    #[tokio::test]
    async fn an_episode_that_belongs_to_no_season_is_met_on_its_own() {
        // The fault this guards against is not about series at all: a file
        // dropped into a library of series became an episode with no parent,
        // and the filter that keeps episodes out of grids kept that one out
        // too. The file was scanned, analysed and stored, and no page in the
        // whole server could reach it. Every way of counting has to agree
        // about it, exactly as they agree about a season under its series.
        let (database, library_id) = library_of(&[("Quiet Harbour", 2019, 7.4)]).await;

        let stray = database
            .create_work(
                library_id,
                WorkKind::Episode,
                "Distant Signal, a night nobody numbered",
                "distant signal, a night nobody numbered",
                Some(2019),
            )
            .await
            .expect("work created");

        sqlx::query("INSERT INTO genres (id, name) VALUES ('g1', 'Drame')")
            .execute(database.writer())
            .await
            .expect("genre created");
        sqlx::query("INSERT INTO work_genres (work_id, genre_id) VALUES (?, 'g1')")
            .bind(stray.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("genre attached");

        let page = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                ..Default::default()
            })
            .await
            .expect("read");
        assert!(
            page.cards.iter().any(|card| card.id == stray.id),
            "a file nobody could number still has somewhere to be seen"
        );

        let shown = page.cards.len() as i64;
        assert_eq!(shown, 2, "the film, and the episode belonging to nothing");
        assert_eq!(
            database
                .count_browsable(Some(library_id))
                .await
                .expect("read"),
            shown
        );
        assert_eq!(
            database
                .genres_in_use(Some(library_id))
                .await
                .expect("read"),
            vec![("Drame".to_string(), 1)],
            "and the menus count it once, like any other card"
        );
        assert_eq!(
            database
                .decades_in_use(Some(library_id))
                .await
                .expect("read"),
            vec![(2010, shown)]
        );
        assert_eq!(
            database
                .initials_in_use(Some(library_id))
                .await
                .expect("read")
                .iter()
                .map(|(_, total)| total)
                .sum::<i64>(),
            shown
        );
    }

    /// The ordering title, folded the way the library crate folds it. Repeated
    /// here rather than depended on: this crate sits below that one.
    fn melyxar_library_sort_title(title: &str) -> String {
        title.to_lowercase()
    }

    fn titles(page: &WorkPage) -> Vec<String> {
        page.cards.iter().map(|card| card.title.clone()).collect()
    }

    #[tokio::test]
    async fn a_grid_comes_back_in_the_order_it_was_asked_for() {
        let (database, library_id) = library_of(&[
            ("Quiet Harbour", 2019, 7.4),
            ("Amber Field", 2020, 8.1),
            ("Winter Signal", 1998, 6.2),
        ])
        .await;

        let page = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(
            titles(&page),
            vec!["Amber Field", "Quiet Harbour", "Winter Signal"]
        );

        let by_year = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                order: WorkOrder::ReleaseYear,
                descending: true,
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(
            titles(&by_year),
            vec!["Amber Field", "Quiet Harbour", "Winter Signal"]
        );
    }

    #[tokio::test]
    async fn paging_walks_the_whole_library_once_and_only_once() {
        let films: Vec<(String, i32, f64)> = (0..25)
            .map(|index| (format!("Film {index:02}"), 2000 + index, 5.0))
            .collect();
        let borrowed: Vec<(&str, i32, f64)> = films
            .iter()
            .map(|(title, year, rating)| (title.as_str(), *year, *rating))
            .collect();
        let (database, library_id) = library_of(&borrowed).await;

        let mut seen: Vec<String> = Vec::new();
        let mut after = None;
        loop {
            let page = database
                .browse_works(&BrowseRequest {
                    library_id: Some(library_id),
                    limit: 7,
                    after,
                    ..Default::default()
                })
                .await
                .expect("read");
            seen.extend(titles(&page));
            match page.next {
                Some(cursor) => after = Some(cursor),
                None => break,
            }
        }

        assert_eq!(seen.len(), 25, "every film appears");
        let mut unique = seen.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 25, "and none of them appears twice");
    }

    #[tokio::test]
    async fn the_last_page_says_it_is_the_last() {
        let (database, library_id) =
            library_of(&[("Quiet Harbour", 2019, 7.4), ("Amber Field", 2020, 8.1)]).await;

        let page = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                limit: 10,
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(page.cards.len(), 2);
        assert!(
            page.next.is_none(),
            "a grid must know when to stop asking for more"
        );

        // The case that hides: a library holding exactly one full page. There
        // is nothing after it, and a grid told otherwise asks for a page that
        // does not exist before it admits it has reached the end.
        let exactly_full = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                limit: 2,
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(exactly_full.cards.len(), 2);
        assert!(exactly_full.next.is_none());
    }

    #[tokio::test]
    async fn a_library_says_how_many_works_it_holds() {
        // What the menu and the home page show next to the name, without
        // fetching a page first.
        let (database, library_id) =
            library_of(&[("Quiet Harbour", 2019, 7.4), ("Amber Field", 2020, 8.1)]).await;

        assert_eq!(
            database
                .count_browsable(Some(library_id))
                .await
                .expect("read"),
            2
        );
        assert_eq!(
            database.count_browsable(None).await.expect("read"),
            2,
            "without a library named, the count covers everything"
        );

        let elsewhere = database
            .create_library(
                "Animes",
                melyxar_core::library::LibraryKind::Anime,
                "fr",
                &[(
                    "disk-two".to_string(),
                    std::path::PathBuf::from("/mnt/two/Animes"),
                )],
            )
            .await
            .expect("library created");
        assert_eq!(
            database
                .count_browsable(Some(elsewhere.id))
                .await
                .expect("read"),
            0,
            "a library nobody filled holds nothing, which is not a failure"
        );
    }

    #[tokio::test]
    async fn a_film_added_while_someone_reads_never_doubles_a_card() {
        let (database, library_id) = library_of(&[
            ("Amber Field", 2020, 8.1),
            ("Quiet Harbour", 2019, 7.4),
            ("Winter Signal", 1998, 6.2),
        ])
        .await;

        let first = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                limit: 1,
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(titles(&first), vec!["Amber Field"]);

        // A scan finds a film whose title sorts before everything read so far.
        database
            .create_work(
                library_id,
                WorkKind::Movie,
                "A First Light",
                "a first light",
                Some(2001),
            )
            .await
            .expect("work created");

        let second = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                limit: 1,
                after: first.next,
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(
            titles(&second),
            vec!["Quiet Harbour"],
            "paging from a known row is what stops an insertion shifting every page"
        );
    }

    #[tokio::test]
    async fn a_search_looks_in_the_title_and_nowhere_else() {
        let (database, library_id) = library_of(&[
            ("Quiet Harbour", 2019, 7.4),
            ("The Quiet Ones", 2014, 5.5),
            ("Amber Field", 2020, 8.1),
        ])
        .await;

        let page = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                search: Some("quiet".to_string()),
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(page.cards.len(), 2);
    }

    #[tokio::test]
    async fn a_title_holding_a_pattern_character_matches_itself_and_not_everything() {
        let (database, library_id) =
            library_of(&[("100% Wolf", 2020, 5.9), ("Amber Field", 2019, 8.1)]).await;

        let page = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                search: Some("100%".to_string()),
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(titles(&page), vec!["100% Wolf"]);
    }

    #[tokio::test]
    async fn a_grid_can_be_narrowed_to_one_decade() {
        let (database, library_id) = library_of(&[
            ("Winter Signal", 1998, 6.2),
            ("Quiet Harbour", 2019, 7.4),
            ("Amber Field", 2020, 8.1),
        ])
        .await;

        let page = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                decade: Some(2010),
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(titles(&page), vec!["Quiet Harbour"]);
    }

    #[tokio::test]
    async fn the_decade_a_grid_is_narrowed_by_follows_the_year_it_comes_from() {
        // The decade is worked out by the database and written by nobody,
        // which is the whole reason it can be trusted: a film wrongly dated
        // and corrected later moves to the decade it belongs to, with nothing
        // anywhere having to remember to move it.
        let (database, library_id) = library_of(&[("Winter Signal", 1998, 6.2)]).await;
        let narrowed_to = |decade: i32| BrowseRequest {
            library_id: Some(library_id),
            decade: Some(decade),
            ..Default::default()
        };

        assert_eq!(
            titles(&database.browse_works(&narrowed_to(1990)).await.expect("read")),
            vec!["Winter Signal"]
        );

        sqlx::query("UPDATE works SET release_year = 2003 WHERE title = 'Winter Signal'")
            .execute(database.writer())
            .await
            .expect("year corrected");

        assert!(
            titles(&database.browse_works(&narrowed_to(1990)).await.expect("read")).is_empty(),
            "it has left the decade it was wrongly in"
        );
        assert_eq!(
            titles(&database.browse_works(&narrowed_to(2000)).await.expect("read")),
            vec!["Winter Signal"],
            "and arrived in the one it belongs to"
        );

        sqlx::query("UPDATE works SET release_year = NULL WHERE title = 'Winter Signal'")
            .execute(database.writer())
            .await
            .expect("year cleared");
        assert!(
            database
                .decades_in_use(Some(library_id))
                .await
                .expect("read")
                .is_empty(),
            "a film with no year belongs to no decade and offers none"
        );
    }

    #[tokio::test]
    async fn a_film_with_nothing_to_order_on_does_not_lead_the_grid() {
        let (database, library_id) = library_of(&[("Quiet Harbour", 2019, 7.4)]).await;
        database
            .create_work(library_id, WorkKind::Movie, "Unknown", "unknown", None)
            .await
            .expect("work created");

        let by_rating = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                order: WorkOrder::CommunityRating,
                descending: true,
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(
            titles(&by_rating),
            vec!["Quiet Harbour"],
            "a film with no rating has no place in a list ordered by rating"
        );

        let alphabetical = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(
            alphabetical.cards.len(),
            2,
            "it is still in the library, and alphabetically it has its place"
        );
    }

    #[tokio::test]
    async fn the_films_nobody_recognised_can_be_listed_on_their_own() {
        let (database, library_id) = library_of(&[("Quiet Harbour", 2019, 7.4)]).await;
        database
            .create_work(library_id, WorkKind::Movie, "Unknown", "unknown", None)
            .await
            .expect("work created");

        let page = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                unidentified_only: true,
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(titles(&page), vec!["Unknown"]);
    }

    #[tokio::test]
    async fn a_card_says_why_its_film_has_no_name() {
        let (database, library_id) = library_of(&[]).await;
        let work = database
            .create_work(library_id, WorkKind::Movie, "Unknown", "unknown", None)
            .await
            .expect("work created");
        database
            .set_identification_note(work.id, IdentificationNote::NoMatch)
            .await
            .expect("note written");

        let page = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(
            page.cards[0].identification_note,
            Some(IdentificationNote::NoMatch),
            "a grid that only says a film is nameless sends nobody anywhere"
        );
    }

    #[tokio::test]
    async fn a_grid_can_be_asked_to_start_at_one_letter() {
        // A few hundred films is too long to scroll and too short to search by
        // hand every time. The letter is also the one thing a viewer reliably
        // remembers about a title.
        let (database, library_id) = library_of(&[
            ("Amber Field", 2020, 7.0),
            ("Quiet Harbour", 2019, 7.4),
            ("Zephyr", 2021, 6.0),
            ("2 Lost Days", 2015, 6.5),
        ])
        .await;
        // A leading article is moved out of the way when the ordering title is
        // built, so this film belongs under A and nowhere else.
        database
            .create_work(
                library_id,
                WorkKind::Movie,
                "The Amber Road",
                "amber road",
                Some(2018),
            )
            .await
            .expect("work created");

        let starting_at = async |letter: &str| {
            let page = database
                .browse_works(&BrowseRequest {
                    library_id: Some(library_id),
                    initial: Initial::parse(letter),
                    ..Default::default()
                })
                .await
                .expect("read");
            titles(&page)
        };

        assert_eq!(
            starting_at("a").await,
            vec!["Amber Field", "The Amber Road"],
            "the ordering title is what decides, so a leading article counts for nothing"
        );
        assert_eq!(
            starting_at("A").await,
            vec!["Amber Field", "The Amber Road"]
        );
        assert_eq!(starting_at("q").await, vec!["Quiet Harbour"]);
        assert_eq!(
            starting_at("z").await,
            vec!["Zephyr"],
            "the last letter is a letter"
        );
        assert!(starting_at("b").await.is_empty());
        assert_eq!(
            starting_at("#").await,
            vec!["2 Lost Days"],
            "everything beginning with no letter shares one bucket"
        );
    }

    #[tokio::test]
    async fn the_letters_offered_are_the_ones_the_library_really_has() {
        let (database, library_id) = library_of(&[
            ("Amber Field", 2020, 7.0),
            ("Zephyr", 2021, 6.0),
            ("2 Lost Days", 2015, 6.5),
        ])
        .await;
        database
            .create_work(
                library_id,
                WorkKind::Movie,
                "The Amber Road",
                "amber road",
                Some(2018),
            )
            .await
            .expect("work created");

        assert_eq!(
            database
                .initials_in_use(Some(library_id))
                .await
                .expect("read"),
            vec![
                ("#".to_string(), 1),
                ("a".to_string(), 2),
                ("z".to_string(), 1),
            ],
            "offering a letter nobody has leads to an empty grid and looks like a fault"
        );
    }

    #[test]
    fn only_a_single_letter_is_ever_taken_for_one() {
        assert_eq!(Initial::parse("c"), Some(Initial::Letter('c')));
        assert_eq!(Initial::parse("C"), Some(Initial::Letter('c')));
        assert_eq!(Initial::parse("#"), Some(Initial::Other));
        assert_eq!(Initial::parse(""), None);
        assert_eq!(Initial::parse("ab"), None);
        assert_eq!(Initial::parse("4"), None);
        assert_eq!(Initial::parse("%"), None);
    }

    #[tokio::test]
    async fn one_request_asking_for_everything_is_bounded() {
        let (database, library_id) = library_of(&[("Quiet Harbour", 2019, 7.4)]).await;
        let page = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                limit: 100_000,
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(page.cards.len(), 1, "the bound is a bound, not a failure");
    }

    #[tokio::test]
    async fn a_card_carries_its_poster_in_every_size() {
        let (database, library_id) = library_of(&[("Quiet Harbour", 2019, 7.4)]).await;
        let page = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                ..Default::default()
            })
            .await
            .expect("read");
        let work_id = page.cards[0].id;

        let sizes: Vec<StoredImage> = [200, 400, 800]
            .into_iter()
            .map(|width| StoredImage {
                owner_kind: "work".to_string(),
                owner_id: work_id.to_db_string(),
                image_kind: "poster".to_string(),
                relative_path: format!("works/{work_id}/poster-abc-{width}.webp"),
                width: Some(width),
                height: Some(width * 3 / 2),
                fingerprint: "abc".to_string(),
                dominant_color: Some("#c81e1e".to_string()),
            })
            .collect();
        database
            .replace_images("work", &work_id.to_db_string(), "poster", &sizes)
            .await
            .expect("images stored");

        let page = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(page.cards[0].poster.len(), 3);
        assert_eq!(
            page.cards[0].poster[0].width,
            Some(800),
            "largest first, so a client can pick without sorting"
        );
    }

    #[tokio::test]
    async fn a_filter_menu_is_built_from_what_is_actually_there() {
        let (database, library_id) = library_of(&[("Quiet Harbour", 2019, 7.4)]).await;
        let page = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                ..Default::default()
            })
            .await
            .expect("read");

        sqlx::query("INSERT INTO genres (id, name) VALUES ('g1', 'Drame')")
            .execute(database.writer())
            .await
            .expect("genre created");
        sqlx::query("INSERT INTO work_genres (work_id, genre_id) VALUES (?, 'g1')")
            .bind(page.cards[0].id.to_db_string())
            .execute(database.writer())
            .await
            .expect("genre attached");

        assert_eq!(
            database
                .genres_in_use(Some(library_id))
                .await
                .expect("read"),
            vec![("Drame".to_string(), 1)]
        );
        assert_eq!(
            database
                .decades_in_use(Some(library_id))
                .await
                .expect("read"),
            vec![(2010, 1)]
        );

        let filtered = database
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                genre: Some("drame".to_string()),
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(
            filtered.cards.len(),
            1,
            "a genre is matched whatever its case"
        );
    }

    #[tokio::test]
    async fn a_library_nobody_named_shows_everything() {
        let (database, _) = library_of(&[("Quiet Harbour", 2019, 7.4)]).await;
        let page = database
            .browse_works(&BrowseRequest::default())
            .await
            .expect("read");
        assert_eq!(page.cards.len(), 1);
        assert_eq!(database.count_browsable(None).await.expect("read"), 1);
    }

    #[tokio::test]
    async fn an_empty_library_is_an_empty_page_and_not_a_failure() {
        let database = Database::open_in_memory().await.expect("database opens");
        let page = database
            .browse_works(&BrowseRequest::default())
            .await
            .expect("read");
        assert!(page.cards.is_empty());
        assert!(page.next.is_none());
        assert!(database.genres_in_use(None).await.expect("read").is_empty());
    }
}
