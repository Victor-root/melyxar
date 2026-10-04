//! Requests for titles the server does not hold: whether accounts may make
//! them, which accounts may, each account's requests and what became of
//! them, and which of the titles asked about the libraries already hold.

use std::collections::HashMap;

use melyxar_core::id::{LibraryId, RequestId, UserId, WorkId};
use melyxar_core::time::Timestamp;
use sqlx::{AssertSqlSafe, Row};

use crate::convert::{
    bool_to_int, int_to_bool, parse_id, parse_optional_timestamp, parse_timestamp,
    timestamp_to_text,
};
use crate::{Database, DatabaseError, Result};

/// One request about to be written.
#[derive(Debug, Clone, PartialEq)]
pub struct NewRequest<'a> {
    pub user_id: UserId,
    /// films or series.
    pub catalogue: &'a str,
    pub tmdb_id: &'a str,
    pub title: &'a str,
    pub year: Option<i32>,
    pub poster_path: Option<&'a str>,
    pub overview: Option<&'a str>,
    /// Empty for a film or a whole series.
    pub seasons: &'a [i32],
    pub note: &'a str,
    pub created_at: Timestamp,
}

/// One account's request, as kept.
#[derive(Debug, Clone, PartialEq)]
pub struct TitleRequest {
    pub id: RequestId,
    pub user_id: UserId,
    pub user_name: String,
    pub catalogue: String,
    pub tmdb_id: String,
    pub title: String,
    pub year: Option<i32>,
    pub poster_path: Option<String>,
    pub overview: Option<String>,
    pub seasons: Vec<i32>,
    pub note: String,
    /// pending, accepted, refused, added.
    pub state: String,
    pub answer: String,
    pub work_id: Option<WorkId>,
    pub created_at: Timestamp,
    pub decided_at: Option<Timestamp>,
}

/// A title the libraries hold, as far as asking for it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldTitle {
    pub work_id: WorkId,
    pub library_id: LibraryId,
    /// For a series, the seasons with at least one episode, in order, across
    /// every library holding it.
    pub seasons: Vec<i32>,
}

/// What a request is in, when the administrator decides it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Accepted,
    Refused,
    Added,
}

impl Decision {
    fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Refused => "refused",
            Self::Added => "added",
        }
    }
}

const WHAT_A_REQUEST_IS: &str = "r.id, r.user_id, u.name AS user_name, r.catalogue, r.tmdb_id, \
     r.title, r.year, r.poster_path, r.overview, r.seasons, r.note, r.state, r.answer, r.work_id, \
     r.created_at, r.decided_at";

const OPEN: &str = "r.state IN ('pending', 'accepted')";

fn request_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<TitleRequest> {
    let seasons: String = row.try_get("seasons")?;
    Ok(TitleRequest {
        id: parse_id(&row.try_get::<String, _>("id")?)?,
        user_id: parse_id(&row.try_get::<String, _>("user_id")?)?,
        user_name: row.try_get("user_name")?,
        catalogue: row.try_get("catalogue")?,
        tmdb_id: row.try_get("tmdb_id")?,
        title: row.try_get("title")?,
        year: row.try_get("year")?,
        poster_path: row.try_get("poster_path")?,
        overview: row.try_get("overview")?,
        seasons: serde_json::from_str(&seasons)
            .map_err(|error| DatabaseError::Corrupt(format!("seasons of a request: {error}")))?,
        note: row.try_get("note")?,
        state: row.try_get("state")?,
        answer: row.try_get("answer")?,
        work_id: row
            .try_get::<Option<String>, _>("work_id")?
            .as_deref()
            .map(parse_id)
            .transpose()?,
        created_at: parse_timestamp(&row.try_get::<String, _>("created_at")?)?,
        decided_at: parse_optional_timestamp(
            row.try_get::<Option<String>, _>("decided_at")?.as_deref(),
        )?,
    })
}

/// The kind of work a catalogue holds.
fn kind_of(catalogue: &str) -> &'static str {
    match catalogue {
        "series" => "series",
        _ => "movie",
    }
}

impl Database {
    pub async fn requests_enabled(&self) -> Result<bool> {
        let enabled: i64 = sqlx::query_scalar("SELECT enabled FROM request_settings WHERE id = 1")
            .fetch_one(self.reader())
            .await?;
        Ok(int_to_bool(enabled))
    }

    pub async fn set_requests_enabled(&self, enabled: bool) -> Result<()> {
        sqlx::query("UPDATE request_settings SET enabled = ? WHERE id = 1")
            .bind(bool_to_int(enabled))
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// The accounts given the right to ask.
    pub async fn request_rights(&self) -> Result<Vec<UserId>> {
        let ids: Vec<String> = sqlx::query_scalar("SELECT user_id FROM request_rights")
            .fetch_all(self.reader())
            .await?;
        ids.iter().map(|id| parse_id(id)).collect()
    }

    pub async fn set_request_right(&self, user: UserId, may: bool) -> Result<()> {
        let query = match may {
            true => "INSERT OR IGNORE INTO request_rights (user_id) VALUES (?)",
            false => "DELETE FROM request_rights WHERE user_id = ?",
        };
        sqlx::query(query)
            .bind(user.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Writes a request and answers it as kept. An account asking again for
    /// a title it already asked for and still waits on is refused as a
    /// duplicate.
    pub async fn add_request(&self, new: &NewRequest<'_>) -> Result<TitleRequest> {
        let id = RequestId::new();
        let seasons = serde_json::to_string(new.seasons)
            .map_err(|error| DatabaseError::Corrupt(error.to_string()))?;
        sqlx::query(
            "INSERT INTO title_requests (id, user_id, catalogue, tmdb_id, title, year, poster_path,
                 overview, seasons, note, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(new.user_id.to_db_string())
        .bind(new.catalogue)
        .bind(new.tmdb_id)
        .bind(new.title)
        .bind(new.year)
        .bind(new.poster_path)
        .bind(new.overview)
        .bind(seasons)
        .bind(new.note)
        .bind(timestamp_to_text(new.created_at))
        .execute(self.writer())
        .await?;
        self.request(id)
            .await?
            .ok_or_else(|| DatabaseError::Corrupt("a request just written is missing".to_string()))
    }

    pub async fn request(&self, id: RequestId) -> Result<Option<TitleRequest>> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_REQUEST_IS} FROM title_requests r JOIN users u ON u.id = r.user_id
              WHERE r.id = ?"
        )))
        .bind(id.to_db_string())
        .fetch_optional(self.writer())
        .await?;
        row.as_ref().map(request_from_row).transpose()
    }

    /// An account's requests, newest first.
    pub async fn requests_of(&self, user: UserId) -> Result<Vec<TitleRequest>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_REQUEST_IS} FROM title_requests r JOIN users u ON u.id = r.user_id
              WHERE r.user_id = ?
              ORDER BY r.id DESC"
        )))
        .bind(user.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.iter().map(request_from_row).collect()
    }

    /// Every request still waiting on the administrator or on the title,
    /// oldest first.
    pub async fn open_requests(&self) -> Result<Vec<TitleRequest>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_REQUEST_IS} FROM title_requests r JOIN users u ON u.id = r.user_id
              WHERE {OPEN}
              ORDER BY r.id"
        )))
        .fetch_all(self.reader())
        .await?;
        rows.iter().map(request_from_row).collect()
    }

    /// The open requests for some titles of one catalogue.
    pub async fn open_requests_for(
        &self,
        catalogue: &str,
        tmdb_ids: &[String],
    ) -> Result<Vec<TitleRequest>> {
        if tmdb_ids.is_empty() {
            return Ok(Vec::new());
        }
        // Assembled from question marks only.
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_REQUEST_IS} FROM title_requests r JOIN users u ON u.id = r.user_id
              WHERE {OPEN} AND r.catalogue = ? AND r.tmdb_id IN ({})
              ORDER BY r.id",
            vec!["?"; tmdb_ids.len()].join(", ")
        )))
        .bind(catalogue);
        for id in tmdb_ids {
            query = query.bind(id);
        }
        let rows = query.fetch_all(self.reader()).await?;
        rows.iter().map(request_from_row).collect()
    }

    /// Takes back one of an account's requests, unless the administrator
    /// accepted it. Answers whether it went.
    pub async fn withdraw_request(&self, user: UserId, id: RequestId) -> Result<bool> {
        let done = sqlx::query(
            "DELETE FROM title_requests WHERE id = ? AND user_id = ? AND state <> 'accepted'",
        )
        .bind(id.to_db_string())
        .bind(user.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Decides every open request for one title, and answers them decided.
    pub async fn decide_requests(
        &self,
        catalogue: &str,
        tmdb_id: &str,
        decision: Decision,
        answer: &str,
        at: Timestamp,
    ) -> Result<Vec<TitleRequest>> {
        let ids: Vec<String> = sqlx::query_scalar(
            "UPDATE title_requests SET state = ?, answer = ?, decided_at = ?
              WHERE catalogue = ? AND tmdb_id = ? AND state IN ('pending', 'accepted')
              RETURNING id",
        )
        .bind(decision.as_str())
        .bind(answer)
        .bind(timestamp_to_text(at))
        .bind(catalogue)
        .bind(tmdb_id)
        .fetch_all(self.writer())
        .await?;
        self.requests_by_id(&ids).await
    }

    /// Marks some open requests added, as the work they became, and answers
    /// the ones that were still open.
    pub async fn requests_arrived(
        &self,
        ids: &[RequestId],
        work: WorkId,
        at: Timestamp,
    ) -> Result<Vec<TitleRequest>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // Assembled from question marks only.
        let mut query = sqlx::query_scalar::<_, String>(AssertSqlSafe(format!(
            "UPDATE title_requests SET state = 'added', work_id = ?, decided_at = ?
              WHERE state IN ('pending', 'accepted') AND id IN ({})
              RETURNING id",
            vec!["?"; ids.len()].join(", ")
        )))
        .bind(work.to_db_string())
        .bind(timestamp_to_text(at));
        for id in ids {
            query = query.bind(id.to_db_string());
        }
        let changed = query.fetch_all(self.writer()).await?;
        self.requests_by_id(&changed).await
    }

    async fn requests_by_id(&self, ids: &[String]) -> Result<Vec<TitleRequest>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // Assembled from question marks only.
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_REQUEST_IS} FROM title_requests r JOIN users u ON u.id = r.user_id
              WHERE r.id IN ({})
              ORDER BY r.id",
            vec!["?"; ids.len()].join(", ")
        )));
        for id in ids {
            query = query.bind(id);
        }
        let rows = query.fetch_all(self.writer()).await?;
        rows.iter().map(request_from_row).collect()
    }

    /// The works of the libraries that somebody asked for, still waiting or
    /// already added: a film, or a series, identified by the number the
    /// request carries.
    pub async fn requested_works(&self) -> Result<std::collections::HashSet<WorkId>> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT w.id
               FROM title_requests r
               JOIN work_external_ids e ON e.provider = 'tmdb' AND e.external_id = r.tmdb_id
               JOIN works w ON w.id = e.work_id AND w.parent_id IS NULL
                           AND w.kind = CASE r.catalogue WHEN 'series' THEN 'series' ELSE 'movie' END
              WHERE r.state IN ('pending', 'accepted', 'added')",
        )
        .fetch_all(self.reader())
        .await?;
        ids.iter().map(|id| parse_id(id)).collect()
    }

    /// Which of some titles of one catalogue the libraries hold, identified,
    /// by their identifier at the provider. A title held twice is answered
    /// by its oldest copy, with the seasons of every copy.
    pub async fn held_titles(
        &self,
        catalogue: &str,
        tmdb_ids: &[String],
    ) -> Result<HashMap<String, HeldTitle>> {
        if tmdb_ids.is_empty() {
            return Ok(HashMap::new());
        }
        // Assembled from question marks only.
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT e.external_id, w.id, w.library_id,
                    (SELECT json_group_array(s.ordinal) FROM works s
                      WHERE s.parent_id = w.id AND s.kind = 'season' AND s.ordinal IS NOT NULL
                        AND EXISTS (SELECT 1 FROM works ep WHERE ep.parent_id = s.id)) AS seasons
               FROM work_external_ids e
               JOIN works w ON w.id = e.work_id
              WHERE e.provider = 'tmdb' AND w.kind = ? AND w.parent_id IS NULL
                AND w.identification IN ('identified', 'manual')
                AND e.external_id IN ({})
              ORDER BY w.added_at",
            vec!["?"; tmdb_ids.len()].join(", ")
        )))
        .bind(kind_of(catalogue));
        for id in tmdb_ids {
            query = query.bind(id);
        }
        let rows = query.fetch_all(self.reader()).await?;

        let mut held: HashMap<String, HeldTitle> = HashMap::new();
        for row in &rows {
            let external: String = row.try_get("external_id")?;
            let seasons: Vec<i32> = serde_json::from_str(&row.try_get::<String, _>("seasons")?)
                .map_err(|error| DatabaseError::Corrupt(format!("seasons held: {error}")))?;
            let title = held.entry(external).or_insert(HeldTitle {
                work_id: parse_id(&row.try_get::<String, _>("id")?)?,
                library_id: parse_id(&row.try_get::<String, _>("library_id")?)?,
                seasons: Vec::new(),
            });
            title.seasons.extend(seasons);
            title.seasons.sort_unstable();
            title.seasons.dedup();
        }
        Ok(held)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use melyxar_core::library::LibraryKind;
    use melyxar_core::user::Permissions;
    use melyxar_core::work::WorkKind;
    use time::macros::datetime;

    use super::*;

    const NOON: Timestamp = datetime!(2026-10-03 12:00 UTC);

    async fn an_account() -> (Database, UserId, UserId) {
        let database = Database::open_in_memory().await.expect("database opens");
        let one = database
            .create_user("one", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account created");
        let other = database
            .create_user("other", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account created");
        (database, one.id, other.id)
    }

    fn asking(user: UserId, tmdb_id: &'static str, seasons: &'static [i32]) -> NewRequest<'static> {
        NewRequest {
            user_id: user,
            catalogue: if seasons.is_empty() { "films" } else { "series" },
            tmdb_id,
            title: "The Lantern Keeper",
            year: Some(2019),
            poster_path: Some("/lantern.jpg"),
            overview: Some("A keeper and a lantern."),
            seasons,
            note: "the long cut",
            created_at: NOON,
        }
    }

    #[tokio::test]
    async fn the_switch_and_the_rights_are_kept() {
        let (database, one, other) = an_account().await;
        assert!(!database.requests_enabled().await.expect("read"), "off at first");
        database.set_requests_enabled(true).await.expect("written");
        assert!(database.requests_enabled().await.expect("read"));

        database.set_request_right(one, true).await.expect("written");
        database.set_request_right(one, true).await.expect("given twice");
        database.set_request_right(other, true).await.expect("written");
        database.set_request_right(other, false).await.expect("taken back");
        assert_eq!(database.request_rights().await.expect("read"), vec![one]);
    }

    #[tokio::test]
    async fn a_request_is_kept_once_while_open_and_read_back_whole() {
        let (database, one, other) = an_account().await;
        let kept = database.add_request(&asking(one, "11", &[])).await.expect("written");
        assert_eq!(kept.user_name, "one");
        assert_eq!(kept.state, "pending");
        assert_eq!(kept.note, "the long cut");
        assert_eq!(kept.poster_path.as_deref(), Some("/lantern.jpg"));

        let again = database.add_request(&asking(one, "11", &[])).await;
        assert!(again.expect_err("refused").is_a_duplicate());
        database.add_request(&asking(other, "11", &[])).await.expect("another account may");
        let series = database.add_request(&asking(one, "11", &[2, 3])).await.expect("another catalogue");
        assert_eq!(series.seasons, vec![2, 3]);

        let mine = database.requests_of(one).await.expect("read");
        assert_eq!(mine.iter().map(|one| one.id).collect::<Vec<_>>(), vec![series.id, kept.id]);
        assert_eq!(database.open_requests().await.expect("read").len(), 3);
        let for_title = database
            .open_requests_for("films", &["11".to_string(), "12".to_string()])
            .await
            .expect("read");
        assert_eq!(for_title.len(), 2);
    }

    #[tokio::test]
    async fn a_decision_reaches_every_open_request_of_the_title() {
        let (database, one, other) = an_account().await;
        database.add_request(&asking(one, "11", &[])).await.expect("written");
        database.add_request(&asking(other, "11", &[])).await.expect("written");
        database.add_request(&asking(one, "12", &[])).await.expect("written");

        let accepted = database
            .decide_requests("films", "11", Decision::Accepted, "", NOON)
            .await
            .expect("decided");
        assert_eq!(accepted.len(), 2);
        assert!(accepted.iter().all(|one| one.state == "accepted" && one.decided_at == Some(NOON)));

        let refused = database
            .decide_requests("films", "11", Decision::Refused, "Not to be found", NOON)
            .await
            .expect("decided");
        assert!(refused.iter().all(|one| one.answer == "Not to be found"));
        let none = database
            .decide_requests("films", "11", Decision::Added, "", NOON)
            .await
            .expect("decided");
        assert!(none.is_empty(), "a refused request stays refused");
        database.add_request(&asking(one, "11", &[])).await.expect("asked again once refused");
    }

    #[tokio::test]
    async fn an_accepted_request_may_not_be_withdrawn_and_others_may() {
        let (database, one, other) = an_account().await;
        let film = database.add_request(&asking(one, "11", &[])).await.expect("written");
        let other_film = database.add_request(&asking(one, "12", &[])).await.expect("written");
        database
            .decide_requests("films", "12", Decision::Accepted, "", NOON)
            .await
            .expect("decided");

        assert!(!database.withdraw_request(other, film.id).await.expect("asked"), "not its own");
        assert!(!database.withdraw_request(one, other_film.id).await.expect("asked"));
        assert!(database.withdraw_request(one, film.id).await.expect("asked"));
        assert_eq!(database.requests_of(one).await.expect("read").len(), 1);
    }

    #[tokio::test]
    async fn held_titles_are_found_by_their_identifier_with_the_seasons_that_have_episodes() {
        let (database, one, _) = an_account().await;
        let library = database
            .create_library("Series", LibraryKind::Series, "en", &[("disk".to_string(), PathBuf::from("/series"))])
            .await
            .expect("library");
        let series = database
            .create_work(library.id, WorkKind::Series, "The Lantern Keeper", "lantern keeper", None)
            .await
            .expect("series");
        for (number, episodes) in [(1, 2), (2, 0), (3, 1)] {
            let season = database
                .create_child_work(library.id, series.id, number, WorkKind::Season, "Season", "season")
                .await
                .expect("season");
            for episode in 1..=episodes {
                database
                    .create_child_work(library.id, season.id, episode, WorkKind::Episode, "Episode", "episode")
                    .await
                    .expect("episode");
            }
        }
        database
            .set_work_external_id(series.id, "tmdb", "40")
            .await
            .expect("named");
        let asked = vec!["40".to_string(), "41".to_string()];
        assert!(
            database.held_titles("series", &asked).await.expect("read").is_empty(),
            "a work not yet identified is not held"
        );

        sqlx::query("UPDATE works SET identification = 'identified' WHERE id = ?")
            .bind(series.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("identified");
        let held = database.held_titles("series", &asked).await.expect("read");
        assert_eq!(
            held.get("40"),
            Some(&HeldTitle {
                work_id: series.id,
                library_id: library.id,
                seasons: vec![1, 3],
            })
        );
        assert!(
            database.held_titles("films", &asked).await.expect("read").is_empty(),
            "a film and a series do not share their identifiers"
        );

        assert!(database.requested_works().await.expect("read").is_empty());
        let request = database.add_request(&asking(one, "40", &[1])).await.expect("written");
        assert_eq!(
            database.requested_works().await.expect("read"),
            std::collections::HashSet::from([series.id]),
            "a series is found by the number its request carries"
        );
        let added = database
            .requests_arrived(&[request.id], series.id, NOON)
            .await
            .expect("arrived");
        assert_eq!(added[0].state, "added");
        assert_eq!(added[0].work_id, Some(series.id));
        assert!(
            database.requests_arrived(&[request.id], series.id, NOON).await.expect("asked").is_empty(),
            "arrives once"
        );
    }
}
