//! What the server has been saying, kept so a screen can sort it.
//!
//! Everything already goes to the system journal, which is the right place for
//! a server that has crashed. It is the wrong place for the ordinary question,
//! which is "I pressed play, it did not work, what did you see": that asks
//! somebody to know the name of a service, the syntax of a filter and the
//! shape of a date, and to do it on a machine they may not be sitting at.
//!
//! So the last few thousand lines are kept here too, each carrying a tag: one
//! word saying which part of the server wrote it. A screen ticks the tags it
//! wants and hands the result over ready to paste. That is what lets one
//! person say "send me `subtitles` and `playback`" and the other send exactly
//! that, without either of them explaining anything.
//!
//! Two rules make it worth having:
//!
//! * the tag is read off the code that wrote the line, never set by hand, so
//!   it is always right and nobody has to remember it;
//! * every line is kept whatever its level. A line that has to be switched on
//!   is never there the evening it is wanted. The level decides how a line is
//!   found, not whether it exists.
//!
//! In memory and not in a file on purpose. A file means a folder, permissions,
//! rotation and something to clear out, for a need that lasts as long as one
//! test: press play, copy, paste. What a crash leaves behind is the system
//! journal's business, and it still gets everything.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, OnceLock};

use crate::time::{now, Timestamp};

/// How many lines are kept. Beyond that the oldest goes.
///
/// A few thousand covers the test somebody is running right now, which is what
/// this is for, at a cost of well under a megabyte.
const KEPT: usize = 4_000;

/// The tag a line carries, read from the module that wrote it.
///
/// One word, in the vocabulary of the thing rather than of the code: somebody
/// reading a screen wants `subtitles`, not `melyxar_app::subtitles`. Longest
/// match first, so a narrow module is never swallowed by the crate around it.
pub fn tag_of(module: &str) -> &'static str {
    const BY_MODULE: &[(&str, &str)] = &[
        ("melyxar_app::scan", "scan"),
        // The two heavy readings of a film, whether a scan does them or the
        // nightly upkeep does. Kept apart from `scan` on purpose: the question
        // somebody asks is "why has my bar no pictures", and the answer is in
        // these lines rather than in the thousands a scan writes around them.
        ("melyxar_app::upkeep", "upkeep"),
        // Finding the opening and closing titles of a season by listening to
        // its episodes. A tag of its own rather than a corner of `upkeep`,
        // because it is the one reading whose answer is a judgement about the
        // films rather than a number: what it decided, for which season, and
        // on how much agreement is exactly what somebody sends over when a
        // skip button turns up in the wrong place.
        ("melyxar_app::openings", "openings"),
        ("melyxar_app::identify", "identify"),
        ("melyxar_app::subtitles", "subtitles"),
        ("melyxar_app::playback", "playback"),
        ("melyxar_app::images", "images"),
        ("melyxar_app::startup", "startup"),
        ("melyxar_app::diagnostics", "report"),
        ("melyxar_server::playback", "playback"),
        ("melyxar_server::jobs", "jobs"),
        // What the browser itself saw, kept apart from everything the server
        // saw: the two disagreeing is the whole reason the page says anything.
        ("melyxar_server::page", "page"),
        // How long each answer took. Apart from `http` so that somebody
        // chasing a slow page can ask for these lines alone, and so that
        // somebody chasing anything else can leave them out.
        ("melyxar_server::timing", "timing"),
        ("melyxar_server", "http"),
        ("melyxar_streaming", "streaming"),
        ("melyxar_ffmpeg::hardware", "card"),
        ("melyxar_ffmpeg::subtitles", "subtitles"),
        // Reading the sound of an episode belongs with what it was read for,
        // the same way pulling a subtitle out belongs with subtitles.
        ("melyxar_ffmpeg::sound", "openings"),
        ("melyxar_ffmpeg", "ffmpeg"),
        ("melyxar_playback", "playback"),
        ("melyxar_library", "library"),
        ("melyxar_media_probe", "probe"),
        ("melyxar_metadata", "metadata"),
        ("melyxar_database", "database"),
        ("melyxar_jobs", "jobs"),
        ("melyxar_config", "config"),
    ];
    BY_MODULE
        .iter()
        .find(|(prefix, _)| module.starts_with(prefix))
        .map(|(_, tag)| *tag)
        // The binary itself, and anything nobody has classified yet. Still
        // kept, still copied: an unclassified line is worth more than none.
        .unwrap_or("melyxar")
}

/// One line the server said.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Line {
    pub at: Timestamp,
    /// error, warn, info, debug or trace.
    pub level: &'static str,
    /// One word for the part of the server that wrote it.
    pub tag: &'static str,
    /// The module it came from, which is what finds the line in the source
    /// once somebody is reading it.
    pub module: String,
    pub message: String,
}

fn kept() -> &'static Mutex<VecDeque<Line>> {
    static KEPT_LINES: OnceLock<Mutex<VecDeque<Line>>> = OnceLock::new();
    KEPT_LINES.get_or_init(|| Mutex::new(VecDeque::with_capacity(KEPT)))
}

/// Takes the lines, whatever a poisoned lock says.
///
/// A poisoned lock means a thread panicked while holding it. The journal is a
/// comfort, never a result: carrying on with the lines is better than taking
/// the server down over the place they are written.
fn take() -> std::sync::MutexGuard<'static, VecDeque<Line>> {
    kept()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Keeps one line, dropping the oldest once the room is full.
pub fn remember(level: &'static str, module: &str, message: String) {
    let line = Line {
        at: now(),
        level,
        tag: tag_of(module),
        module: module.to_string(),
        message,
    };
    let mut lines = take();
    if lines.len() == KEPT {
        lines.pop_front();
    }
    lines.push_back(line);
}

/// What a screen asks for.
#[derive(Debug, Clone, Default)]
pub struct Wanted {
    /// Keep only these tags. Empty keeps every tag, which is what somebody who
    /// has not ticked anything means.
    pub tags: Vec<String>,
    /// Keep only lines holding this text, compared without case.
    pub holding: Option<String>,
    /// At most this many, counted from the newest.
    pub most: usize,
}

/// The lines kept, oldest first, narrowed to what was asked for.
pub fn lines(wanted: &Wanted) -> Vec<Line> {
    let lines = take();
    let holding = wanted.holding.as_ref().map(|text| text.to_lowercase());

    let matching: Vec<&Line> = lines
        .iter()
        .filter(|line| wanted.tags.is_empty() || wanted.tags.iter().any(|tag| tag == line.tag))
        .filter(|line| {
            holding
                .as_ref()
                .is_none_or(|text| line.message.to_lowercase().contains(text))
        })
        .collect();

    // Taken from the end: what somebody wants is what just happened, not the
    // first lines of a server that has been up for a week.
    let most = if wanted.most == 0 {
        matching.len()
    } else {
        wanted.most
    };
    matching
        .into_iter()
        .rev()
        .take(most)
        .rev()
        .cloned()
        .collect()
}

/// Every tag that has actually been written, with how many lines carry it.
///
/// What a screen puts on its buttons: the tags this server really has, rather
/// than a list written somewhere that drifts from the code.
pub fn tags() -> BTreeMap<&'static str, usize> {
    let mut counted = BTreeMap::new();
    for line in take().iter() {
        *counted.entry(line.tag).or_insert(0) += 1;
    }
    counted
}

/// Forgets everything kept.
///
/// What a screen offers before a test, so that what is copied afterwards is
/// about that test and nothing else.
pub fn forget() -> usize {
    let mut lines = take();
    let had = lines.len();
    lines.clear();
    had
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Held by every test that empties the journal and writes into it.
    ///
    /// There is one journal for the whole process, and tests run beside each
    /// other: without this, one test empties what another has just written and
    /// the failure lands on whichever of them looked second. Measured: one run
    /// in six. A test that fails for a reason that is not the code teaches the
    /// wrong lesson, which is to run it again.
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Empties the journal and keeps every other test out until this one ends.
    fn alone() -> std::sync::MutexGuard<'static, ()> {
        // A test that fails while holding it poisons it, and every test after
        // would then fail for that reason instead of its own.
        let held = ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(|held| held.into_inner());
        forget();
        held
    }

    #[test]
    fn a_line_is_tagged_with_the_part_of_the_server_that_wrote_it() {
        assert_eq!(tag_of("melyxar_streaming::session"), "streaming");
        assert_eq!(tag_of("melyxar_app::subtitles"), "subtitles");
        assert_eq!(tag_of("melyxar_app::scan"), "scan");
        assert_eq!(tag_of("melyxar_library::naming"), "library");
        assert_eq!(tag_of("melyxar_metadata::tmdb"), "metadata");
    }

    #[test]
    fn a_narrower_module_wins_over_the_crate_around_it() {
        // The defect this guards: read in the wrong order, every line of a
        // crate lands under one tag and the tags stop meaning anything.
        assert_eq!(tag_of("melyxar_app::playback"), "playback");
        assert_eq!(tag_of("melyxar_app::startup"), "startup");
        assert_eq!(tag_of("melyxar_app::upkeep"), "upkeep");
        assert_eq!(tag_of("melyxar_app::openings"), "openings");
        assert_eq!(
            tag_of("melyxar_ffmpeg::sound"),
            "openings",
            "reading the sound of an episode belongs with what it was read for"
        );
        assert_eq!(tag_of("melyxar_ffmpeg::hardware"), "card");
        assert_eq!(tag_of("melyxar_ffmpeg::command"), "ffmpeg");
        assert_eq!(tag_of("melyxar_server::playback"), "playback");
        assert_eq!(tag_of("melyxar_server::works"), "http");
    }

    #[test]
    fn a_module_nobody_has_classified_is_still_kept() {
        assert_eq!(tag_of("something_new"), "melyxar");
    }

    #[test]
    fn a_screen_asking_for_two_tags_gets_those_two_and_nothing_else() {
        let _alone = alone();
        remember("info", "melyxar_app::subtitles", "a subtitle".into());
        remember("warn", "melyxar_app::scan", "a scan".into());
        remember("info", "melyxar_streaming::session", "a segment".into());

        let picked = lines(&Wanted {
            tags: vec!["subtitles".into(), "streaming".into()],
            ..Wanted::default()
        });
        assert_eq!(picked.len(), 2);
        assert_eq!(picked[0].message, "a subtitle");
        assert_eq!(picked[1].message, "a segment", "oldest first, as written");

        assert!(
            lines(&Wanted::default()).len() >= 3,
            "ticking nothing means everything, not nothing"
        );
    }

    #[test]
    fn what_is_asked_for_can_be_narrowed_to_a_word_whatever_its_case() {
        let _alone = alone();
        remember(
            "warn",
            "melyxar_app::subtitles",
            "Picture size is invalid".into(),
        );
        remember(
            "info",
            "melyxar_app::subtitles",
            "a subtitle is ready".into(),
        );

        let picked = lines(&Wanted {
            holding: Some("PICTURE".into()),
            ..Wanted::default()
        });
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].message, "Picture size is invalid");
    }

    #[test]
    fn only_the_newest_are_handed_over_when_a_number_is_given() {
        let _alone = alone();
        for line in 0..10 {
            remember("info", "melyxar_app::scan", format!("line {line}"));
        }
        let picked = lines(&Wanted {
            most: 3,
            ..Wanted::default()
        });
        assert_eq!(picked.len(), 3);
        assert_eq!(picked[0].message, "line 7", "the newest three, in order");
        assert_eq!(picked[2].message, "line 9");
    }

    #[test]
    fn the_tags_offered_are_the_ones_really_written() {
        let _alone = alone();
        remember("info", "melyxar_app::scan", "one".into());
        remember("info", "melyxar_app::scan", "two".into());
        remember("info", "melyxar_streaming::session", "three".into());

        let tags = tags();
        assert_eq!(tags.get("scan"), Some(&2));
        assert_eq!(tags.get("streaming"), Some(&1));
        assert_eq!(
            tags.get("metadata"),
            None,
            "a tag nothing has written is not offered"
        );
    }
}
