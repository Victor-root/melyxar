//! A work's details written by hand, and which of them no look up may undo.

use melyxar_core::id::{LibraryId, WorkId};
pub use melyxar_database::hand_edits::{WrittenCredit, WrittenDetails, EDITABLE_FIELDS};

use crate::state::AppState;
use crate::{AppError, Result};

/// The first and last years a work may say it came out.
const EARLIEST_YEAR: i32 = 1850;
const LATEST_YEAR: i32 = 2200;
/// The highest rating a work may carry, the scale the provider uses.
const HIGHEST_RATING: f64 = 10.0;
/// What somebody may be credited as, the same roles a provider credits.
pub const ROLES: [&str; 5] = ["actor", "director", "writer", "producer", "composer"];

/// What a work says about itself, and which fields are locked.
pub struct DetailsHeld {
    pub details: WrittenDetails,
    pub locked: Vec<String>,
}

/// The language a library is described in, which is the one its pages read
/// first.
pub(crate) async fn language_of(state: &AppState, library_id: LibraryId) -> Result<String> {
    Ok(state
        .database()
        .list_libraries()
        .await?
        .into_iter()
        .find(|library| library.id == library_id)
        .map(|library| library.metadata_language)
        .unwrap_or_else(|| "fr".to_string()))
}

/// What a work says about itself today, ready to be written over.
pub async fn details_of(state: &AppState, work_id: WorkId) -> Result<Option<DetailsHeld>> {
    let database = state.database();
    let Some(work) = database.work(work_id).await? else {
        return Ok(None);
    };
    let language = language_of(state, work.library_id).await?;
    let Some(details) = database.written_details(work_id, &language).await? else {
        return Ok(None);
    };
    let locked = database
        .locked_fields(work_id)
        .await?
        .into_iter()
        .filter(|field| EDITABLE_FIELDS.contains(&field.as_str()))
        .collect();
    Ok(Some(DetailsHeld { details, locked }))
}

/// Writes what a person gave a work, locking the fields they asked to keep.
///
/// Answers false for a work that is not there.
pub async fn write(
    state: &AppState,
    work_id: WorkId,
    given: WrittenDetails,
    locked: &[String],
) -> Result<bool> {
    let database = state.database();
    let Some(work) = database.work(work_id).await? else {
        return Ok(false);
    };
    let details = tidied(given).map_err(|detail| {
        AppError::Domain(melyxar_core::Error::invalid_input(detail))
    })?;
    let locked = fields_kept(locked);
    let language = language_of(state, work.library_id).await?;
    let sort_title = melyxar_library::naming::sort_title(&details.title);

    database
        .write_details(work_id, &language, &details, &sort_title, &locked)
        .await?;
    database.bump_library_version(work.library_id).await?;
    tracing::info!(title = %details.title, locked = ?locked, "details written by hand");
    Ok(true)
}

/// The fields asked to be locked, among those that may be. The year goes
/// with the day it came out, which it is read from: a day kept as written
/// keeps its year with it.
fn fields_kept(asked: &[String]) -> Vec<&'static str> {
    let asked = |field: &str| asked.iter().any(|one| one == field);
    EDITABLE_FIELDS
        .into_iter()
        .filter(|field| asked(field) || (*field == "release_year" && asked("release_date")))
        .collect()
}

/// What a person typed, trimmed, with an empty text read as none and each
/// genre or studio kept once, or why it cannot be kept.
fn tidied(given: WrittenDetails) -> std::result::Result<WrittenDetails, &'static str> {
    let title = given.title.trim().to_string();
    if title.is_empty() {
        return Err("a work needs a title");
    }
    let release_date = some_text(given.release_date);
    let end_date = some_text(given.end_date);
    for day in [&release_date, &end_date].into_iter().flatten() {
        if !melyxar_core::time::is_a_day(day) {
            return Err("a day is written year, month, day");
        }
    }
    if let (Some(start), Some(end)) = (&release_date, &end_date) {
        if end < start {
            return Err("a work cannot end before it came out");
        }
    }
    // The day, when there is one, says the year.
    let release_year = release_date
        .as_deref()
        .and_then(melyxar_core::time::year_of_day)
        .or(given.release_year);
    if let Some(year) = release_year {
        if !(EARLIEST_YEAR..=LATEST_YEAR).contains(&year) {
            return Err("that year is not one a work came out in");
        }
    }
    if let Some(rating) = given.community_rating {
        if !(0.0..=HIGHEST_RATING).contains(&rating) {
            return Err("a rating is between 0 and 10");
        }
    }
    Ok(WrittenDetails {
        title,
        tagline: some_text(given.tagline),
        overview: some_text(given.overview),
        release_year,
        release_date,
        end_date,
        community_rating: given.community_rating,
        age_rating_label: some_text(given.age_rating_label),
        genres: names(given.genres),
        studios: names(given.studios),
        credits: credits(given.credits)?,
    })
}

/// The people credited, each named, in a role there is, playing somebody
/// only as an actor, and listed once for each role.
fn credits(given: Vec<WrittenCredit>) -> std::result::Result<Vec<WrittenCredit>, &'static str> {
    let mut kept: Vec<WrittenCredit> = Vec::new();
    for credit in given {
        let name = credit.name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        if !ROLES.contains(&credit.role.as_str()) {
            return Err("nobody is credited in that role");
        }
        let character = match credit.role.as_str() {
            "actor" => some_text(credit.character),
            _ => None,
        };
        if !kept.iter().any(|held| held.name == name && held.role == credit.role) {
            kept.push(WrittenCredit { name, role: credit.role, character });
        }
    }
    Ok(kept)
}

fn some_text(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn names(given: Vec<String>) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for name in given {
        let name = name.trim().to_string();
        if !name.is_empty() && !kept.iter().any(|held| held.eq_ignore_ascii_case(&name)) {
            kept.push(name);
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed() -> WrittenDetails {
        WrittenDetails {
            title: "  Le Port  ".to_string(),
            tagline: Some("   ".to_string()),
            overview: Some(" Une nuit. ".to_string()),
            release_year: Some(2019),
            release_date: None,
            end_date: None,
            community_rating: Some(7.5),
            age_rating_label: Some(String::new()),
            genres: vec![" Drame".to_string(), "drame".to_string(), String::new()],
            studios: vec!["Studio".to_string()],
            credits: vec![
                WrittenCredit {
                    name: " Alix Moreau ".to_string(),
                    role: "actor".to_string(),
                    character: Some(" Lou ".to_string()),
                },
                WrittenCredit {
                    name: "Alix Moreau".to_string(),
                    role: "actor".to_string(),
                    character: None,
                },
                WrittenCredit {
                    name: "Nora Vidal".to_string(),
                    role: "director".to_string(),
                    character: Some("nobody".to_string()),
                },
                WrittenCredit {
                    name: "  ".to_string(),
                    role: "writer".to_string(),
                    character: None,
                },
            ],
        }
    }

    #[test]
    fn what_is_typed_is_trimmed_and_an_empty_text_is_none() {
        let kept = tidied(typed()).expect("kept");
        assert_eq!(kept.title, "Le Port");
        assert_eq!(kept.tagline, None);
        assert_eq!(kept.overview.as_deref(), Some("Une nuit."));
        assert_eq!(kept.age_rating_label, None);
        assert_eq!(kept.genres, vec!["Drame"]);
        assert_eq!(
            kept.credits,
            vec![
                WrittenCredit {
                    name: "Alix Moreau".to_string(),
                    role: "actor".to_string(),
                    character: Some("Lou".to_string()),
                },
                WrittenCredit {
                    name: "Nora Vidal".to_string(),
                    role: "director".to_string(),
                    character: None,
                },
            ]
        );
    }

    #[test]
    fn a_work_without_a_title_or_with_an_impossible_year_or_rating_is_refused() {
        let mut given = typed();
        given.title = "  ".to_string();
        assert!(tidied(given).is_err());

        let mut given = typed();
        given.release_year = Some(20190);
        assert!(tidied(given).is_err());

        let mut given = typed();
        given.community_rating = Some(11.0);
        assert!(tidied(given).is_err());

        let mut given = typed();
        given.credits[0].role = "catering".to_string();
        assert!(tidied(given).is_err());

        let mut given = typed();
        given.release_date = Some("2019-02-30".to_string());
        assert!(tidied(given).is_err());

        let mut given = typed();
        given.release_date = Some("2020-01-01".to_string());
        given.end_date = Some("2019-01-01".to_string());
        assert!(tidied(given).is_err());
    }

    #[test]
    fn the_day_a_work_came_out_says_its_year_and_locks_it_with_it() {
        let mut given = typed();
        given.release_date = Some("2021-05-04".to_string());
        assert_eq!(tidied(given).expect("kept").release_year, Some(2021));
        assert_eq!(
            fields_kept(&["release_date".to_string(), "nonsense".to_string()]),
            vec!["release_year", "release_date"]
        );
    }
}
