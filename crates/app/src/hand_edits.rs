//! A work's details written by hand, and which of them no look up may undo.

use melyxar_core::id::{LibraryId, WorkId};
pub use melyxar_database::hand_edits::{WrittenDetails, EDITABLE_FIELDS};

use crate::state::AppState;
use crate::{AppError, Result};

/// The first and last years a work may say it came out.
const EARLIEST_YEAR: i32 = 1850;
const LATEST_YEAR: i32 = 2200;
/// The highest rating a work may carry, the scale the provider uses.
const HIGHEST_RATING: f64 = 10.0;

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
    let locked: Vec<&str> = EDITABLE_FIELDS
        .into_iter()
        .filter(|field| locked.iter().any(|asked| asked == field))
        .collect();
    let language = language_of(state, work.library_id).await?;
    let sort_title = melyxar_library::naming::sort_title(&details.title);

    database
        .write_details(work_id, &language, &details, &sort_title, &locked)
        .await?;
    database.bump_library_version(work.library_id).await?;
    tracing::info!(title = %details.title, locked = ?locked, "details written by hand");
    Ok(true)
}

/// What a person typed, trimmed, with an empty text read as none and each
/// genre or studio kept once, or why it cannot be kept.
fn tidied(given: WrittenDetails) -> std::result::Result<WrittenDetails, &'static str> {
    let title = given.title.trim().to_string();
    if title.is_empty() {
        return Err("a work needs a title");
    }
    if let Some(year) = given.release_year {
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
        release_year: given.release_year,
        community_rating: given.community_rating,
        age_rating_label: some_text(given.age_rating_label),
        genres: names(given.genres),
        studios: names(given.studios),
    })
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
            community_rating: Some(7.5),
            age_rating_label: Some(String::new()),
            genres: vec![" Drame".to_string(), "drame".to_string(), String::new()],
            studios: vec!["Studio".to_string()],
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
    }
}
