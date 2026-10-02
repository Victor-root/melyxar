//! The words the speech tool heard, put back into lines of subtitle.
//!
//! Left to itself the tool writes blocks of ten seconds or more that run
//! several sentences together, whoever says them, and cuts them wherever its
//! window ends. It cannot tell who is speaking, but a change of speaker nearly
//! always comes with a full stop or a pause, so the lines are cut there from
//! the time of each word instead.

use serde::Deserialize;

/// Silence between two words that ends a line.
const PAUSE_MS: u64 = 600;
/// The most a line holds: two lines of the usual forty-two characters.
const LONGEST_LINE: usize = 84;
/// The longest a line stays on the screen.
const LONGEST_SHOWN_MS: u64 = 7_000;
/// A line too long is cut at a comma when it has said at least this much by
/// then, and where it stands otherwise.
const SHORTEST_BEFORE_COMMA: usize = 30;
/// A line is shown at least this long when what follows leaves the room.
const SHORTEST_SHOWN_MS: u64 = 1_000;

#[derive(Deserialize)]
struct Report {
    #[serde(default)]
    transcription: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    text: String,
    offsets: Offsets,
}

#[derive(Deserialize)]
struct Offsets {
    from: u64,
    to: u64,
}

struct Word {
    text: String,
    from: u64,
    to: u64,
}

struct Line {
    from: u64,
    to: u64,
    text: String,
}

/// The subtitle file of what the report says was heard, or nothing when the
/// report cannot be read. The tool is asked for one word to each entry.
pub fn subrip_of(report: &str) -> Option<String> {
    let report: Report = serde_json::from_str(report).ok()?;
    let words = report.transcription.into_iter().filter_map(|entry| {
        let text = entry.text.trim();
        (!text.is_empty()).then(|| Word {
            text: text.to_string(),
            from: entry.offsets.from,
            to: entry.offsets.to,
        })
    });
    Some(render(&lines_of(words)))
}

fn lines_of(words: impl Iterator<Item = Word>) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut said: Vec<Word> = Vec::new();
    for word in words {
        if let Some(last) = said.last() {
            if word.from.saturating_sub(last.to) > PAUSE_MS {
                let all = said.len();
                put_down(&mut said, all, &mut lines);
            } else if said_with(&said, &word).len() > LONGEST_LINE || word.to - said[0].from > LONGEST_SHOWN_MS {
                let cut = comma_to_cut_at(&said).unwrap_or(said.len());
                put_down(&mut said, cut, &mut lines);
            }
        }
        let ends_a_sentence = ends_a_sentence(&word.text);
        said.push(word);
        if ends_a_sentence {
            let all = said.len();
            put_down(&mut said, all, &mut lines);
        }
    }
    let all = said.len();
    put_down(&mut said, all, &mut lines);
    lines
}

/// Takes the first `count` words off `said` and makes them a line.
fn put_down(said: &mut Vec<Word>, count: usize, lines: &mut Vec<Line>) {
    if count == 0 {
        return;
    }
    let taken: Vec<Word> = said.drain(..count).collect();
    lines.push(Line {
        from: taken[0].from,
        to: taken[taken.len() - 1].to,
        text: text_of(&taken),
    });
}

fn text_of(words: &[Word]) -> String {
    words.iter().map(|word| word.text.as_str()).collect::<Vec<_>>().join(" ")
}

fn said_with(said: &[Word], word: &Word) -> String {
    format!("{} {}", text_of(said), word.text)
}

/// How many words go before the last comma, when that leaves a line worth showing.
fn comma_to_cut_at(said: &[Word]) -> Option<usize> {
    let at = said.iter().rposition(|word| word.text.ends_with(','))?;
    (text_of(&said[..=at]).len() >= SHORTEST_BEFORE_COMMA).then_some(at + 1)
}

/// Whether a word closes a sentence, whatever quote or bracket comes after the stop.
fn ends_a_sentence(word: &str) -> bool {
    word.trim_end_matches(['"', '\'', '”', '’', ')', ']', '»'])
        .ends_with(['.', '?', '!', '…', '。', '？', '！'])
}

/// A line too brief to read stays up a little longer, as far as the next one allows.
fn render(lines: &[Line]) -> String {
    let mut written = String::new();
    for (position, line) in lines.iter().enumerate() {
        let room = lines.get(position + 1).map_or(u64::MAX, |next| next.from);
        let to = line.to.max((line.from + SHORTEST_SHOWN_MS).min(room));
        written.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            position + 1,
            timestamp(line.from),
            timestamp(to),
            line.text
        ));
    }
    written
}

fn timestamp(milliseconds: u64) -> String {
    format!(
        "{:02}:{:02}:{:02},{:03}",
        milliseconds / 3_600_000,
        milliseconds / 60_000 % 60,
        milliseconds / 1_000 % 60,
        milliseconds % 1_000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A report of words, each given as its text and when it starts and ends.
    fn report(words: &[(&str, u64, u64)]) -> String {
        let entries: Vec<_> = words
            .iter()
            .map(|(text, from, to)| serde_json::json!({"text": format!(" {text}"), "offsets": {"from": from, "to": to}}))
            .collect();
        serde_json::json!({"result": {"language": "en"}, "transcription": entries}).to_string()
    }

    fn texts(subrip: &str) -> Vec<&str> {
        subrip.lines().filter(|line| !line.is_empty() && !line.contains(" --> ") && line.parse::<u32>().is_err()).collect()
    }

    #[test]
    fn two_sentences_said_one_after_the_other_are_two_lines() {
        // The case of a dialogue: nothing between the two voices, only a full stop.
        let written = subrip_of(&report(&[
            ("Where", 0, 300),
            ("were", 300, 500),
            ("you?", 500, 900),
            ("At", 900, 1100),
            ("home.", 1100, 1700),
        ]))
        .expect("a report");
        assert_eq!(texts(&written), ["Where were you?", "At home."]);
    }

    #[test]
    fn a_pause_ends_a_line_even_without_a_full_stop() {
        let written = subrip_of(&report(&[("Well", 0, 400), ("I", 2_000, 2_200), ("see", 2_200, 2_600)])).expect("a report");
        assert_eq!(texts(&written), ["Well", "I see"]);
    }

    #[test]
    fn a_sentence_that_runs_on_is_cut_at_a_comma_rather_than_in_the_middle_of_a_phrase() {
        let long = "this is a rather long beginning to the sentence,";
        let mut words = Vec::new();
        let mut at = 0;
        for word in long.split(' ').chain("and then an equally long ending that keeps going on".split(' ')) {
            words.push((word, at, at + 200));
            at += 200;
        }
        let written = subrip_of(&report(&words)).expect("a report");
        let lines = texts(&written);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("sentence,"), "{lines:?}");
        assert!(lines.iter().all(|line| line.len() <= LONGEST_LINE));
    }

    #[test]
    fn a_line_that_stays_on_too_long_is_cut() {
        let words: Vec<(&str, u64, u64)> = (0..12).map(|index| ("slowly", index * 800, index * 800 + 800)).collect();
        let written = subrip_of(&report(&words)).expect("a report");
        assert!(texts(&written).len() >= 2);
    }

    #[test]
    fn a_brief_line_stays_up_as_long_as_the_next_one_leaves_room() {
        let written = subrip_of(&report(&[("No.", 0, 300), ("Yes.", 800, 1_100), ("Fine.", 5_000, 5_400)])).expect("a report");
        assert!(written.contains("00:00:00,000 --> 00:00:00,800"), "{written}");
        assert!(written.contains("00:00:00,800 --> 00:00:01,800"), "{written}");
        assert!(written.contains("00:00:05,000 --> 00:00:06,000"), "{written}");
    }

    #[test]
    fn what_follows_a_full_stop_inside_a_quote_starts_a_new_line() {
        let written = subrip_of(&report(&[("He", 0, 200), ("said", 200, 400), ("\"go.\"", 400, 800), ("Then", 800, 1_000), ("left.", 1_000, 1_400)])).expect("a report");
        assert_eq!(texts(&written), ["He said \"go.\"", "Then left."]);
    }

    #[test]
    fn empty_entries_are_ignored_and_nothing_heard_is_an_empty_file() {
        assert_eq!(subrip_of(&report(&[("", 0, 300)])).as_deref(), Some(""));
        assert_eq!(subrip_of(r#"{"result":{"language":"en"}}"#).as_deref(), Some(""));
    }

    #[test]
    fn a_report_that_cannot_be_read_gives_nothing() {
        assert_eq!(subrip_of("not a report"), None);
    }

    #[test]
    fn times_are_written_the_way_a_subtitle_file_does() {
        assert_eq!(timestamp(3_723_456), "01:02:03,456");
    }
}
