//! What the page saw of the lyrics of the song playing, written into the
//! journal under the `lyrics` tag.
//!
//! The server knows which words it sent and nothing of which line the page
//! lit when, so a line that stops following the song cannot be told from the
//! server's side. Like `page`, this is a closed list of facts the server
//! names itself: the page chooses which fact to send and never the words of
//! the line the journal writes, and what it sends of the lyrics is cut short.

use axum::{Json, Router};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::identifiers::parse_work;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/system/journal/lyrics",
        axum::routing::post(what_the_page_saw),
    )
}

/// How much of a line of lyrics is written down.
const ENOUGH_OF_A_LINE: usize = 40;

/// Where the lyrics came from, as the page was told.
#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Source {
    Song,
    Beside,
    Online,
}

impl Source {
    fn as_word(self) -> &'static str {
        match self {
            Self::Song => "in the song",
            Self::Beside => "in a file beside the song",
            Self::Online => "from LRCLIB",
        }
    }
}

/// Why a line was lit.
#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Why {
    /// The first line shown for this song.
    First,
    /// The next line, as the song went on.
    Played,
    /// Any other line: somebody jumped, or lines were skipped.
    AfterAJump,
}

impl Why {
    fn as_word(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Played => "played",
            Self::AfterAJump => "after a jump",
        }
    }
}

/// One fact, named by the server.
#[derive(Debug, Deserialize)]
#[serde(tag = "saw", rename_all = "snake_case")]
enum Seen {
    /// The lyrics the page was handed, and what is odd about them: lines out
    /// of order, lines with nothing in them. A list that is not in the order
    /// it is sung in breaks the search for the line being sung.
    LyricsRead {
        source: Source,
        lines: u32,
        first_at_ms: Option<i64>,
        last_at_ms: Option<i64>,
        out_of_order: u32,
        empty_lines: u32,
        instrumental: bool,
    },
    /// A line was lit. `active` is its number from nought, or minus one
    /// before the first line; the page says where the song was at the time.
    LineLit {
        why: Why,
        position_ms: i64,
        active: i64,
        lines: u32,
        line_at_ms: Option<i64>,
        next_at_ms: Option<i64>,
        text: String,
    },
    /// The line lit is not the one the song is at, twice in a row: what the
    /// page shows and what its own clock says disagree.
    LineNotLit {
        position_ms: i64,
        expected: i64,
        shown: i64,
        expected_at_ms: Option<i64>,
    },
    /// The song is playing and its clock has not moved.
    ClockStoodStill { position_ms: i64, for_ms: u32 },
}

#[derive(Debug, Deserialize)]
struct FromThePage {
    song: String,
    #[serde(flatten)]
    seen: Seen,
}

#[derive(Debug, Serialize)]
struct Written {
    written: bool,
}

fn cut_to(words: &str, most: usize) -> &str {
    match words.char_indices().nth(most) {
        Some((at, _)) => &words[..at],
        None => words,
    }
}

async fn what_the_page_saw(
    _: crate::account::Viewer,
    Json(said): Json<FromThePage>,
) -> Result<Json<Written>> {
    // The identifier is a name from outside: read as one, written down only
    // once it parses.
    let song = tracing::field::display(parse_work(&said.song)?);
    match said.seen {
        Seen::LyricsRead {
            source,
            lines,
            first_at_ms,
            last_at_ms,
            out_of_order,
            empty_lines,
            instrumental,
        } => tracing::debug!(
            song,
            source = source.as_word(),
            lines,
            first_at_ms,
            last_at_ms,
            out_of_order,
            empty_lines,
            instrumental,
            "lyrics: the page was handed the words"
        ),
        Seen::LineLit {
            why,
            position_ms,
            active,
            lines,
            line_at_ms,
            next_at_ms,
            text,
        } => tracing::debug!(
            song,
            why = why.as_word(),
            position_ms,
            active,
            lines,
            line_at_ms,
            next_at_ms,
            text = cut_to(&text, ENOUGH_OF_A_LINE),
            "lyrics: a line was lit"
        ),
        Seen::LineNotLit {
            position_ms,
            expected,
            shown,
            expected_at_ms,
        } => tracing::debug!(
            song,
            position_ms,
            expected,
            shown,
            expected_at_ms,
            "lyrics: the line lit is not the one the song is at"
        ),
        Seen::ClockStoodStill { position_ms, for_ms } => tracing::debug!(
            song,
            position_ms,
            for_ms,
            "lyrics: the song is playing and its clock has not moved"
        ),
    }
    Ok(Json(Written { written: true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }

    #[test]
    fn the_facts_are_read_by_their_name_and_anything_else_is_refused() {
        let song = "01a0f0c6-c1fa-769d-9f83-3467c8c4586b";
        let lit = format!(
            r#"{{"song":"{song}","saw":"line_lit","why":"played","position_ms":12000,"active":3,"lines":40,"line_at_ms":11800,"next_at_ms":14000,"text":"words"}}"#
        );
        assert!(matches!(
            serde_json::from_str::<FromThePage>(&lit).expect("read").seen,
            Seen::LineLit { active: 3, .. }
        ));
        for refused in [
            format!(r#"{{"song":"{song}","saw":"anything_else","message":"hi"}}"#),
            format!(r#"{{"song":"{song}","saw":"line_lit","why":"whenever","position_ms":0,"active":0,"lines":1,"text":""}}"#),
        ] {
            assert!(serde_json::from_str::<FromThePage>(&refused).is_err(), "{refused}");
        }
    }

    #[test]
    fn a_line_is_cut_on_a_boundary_between_characters() {
        let accented = "é".repeat(ENOUGH_OF_A_LINE * 2);
        assert_eq!(cut_to(&accented, ENOUGH_OF_A_LINE).chars().count(), ENOUGH_OF_A_LINE);
        assert_eq!(cut_to("short", ENOUGH_OF_A_LINE), "short");
    }
}
