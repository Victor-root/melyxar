//! What the speech tool heard, cut into lines of subtitle.
//!
//! Left to itself the tool writes blocks of ten seconds or more that run
//! several sentences together, whoever says them. It cannot tell who is
//! speaking, but a change of speaker nearly always comes with a full stop, so
//! each block is cut there, and the time of the block shared out among its
//! parts by their length. The block's own times are the only ones trusted: the
//! tool's times for single words are stretched and squeezed around silence and
//! noise, and a line placed by them lands nowhere near where it is said.

use serde::Deserialize;

/// The most a line holds: two lines of the usual forty-two characters.
const LONGEST_LINE: usize = 84;
/// The longest a line stays on the screen, by the pace of the block it is in.
const LONGEST_SHOWN_MS: u64 = 7_000;
/// A line is never made shorter than this to fit the time.
const SHORTEST_LINE: usize = 20;
/// A line too long is cut at a comma when it has said at least this much by
/// then, and where it stands otherwise.
const SHORTEST_BEFORE_COMMA: usize = 30;
/// A line is shown at least this long when what follows leaves the room.
const SHORTEST_SHOWN_MS: u64 = 1_000;

#[derive(Deserialize)]
struct Report {
    #[serde(default)]
    transcription: Vec<Block>,
}

#[derive(Deserialize)]
struct Block {
    text: String,
    offsets: Offsets,
}

#[derive(Deserialize)]
struct Offsets {
    from: u64,
    to: u64,
}

struct Line {
    from: u64,
    to: u64,
    text: String,
}

/// The subtitle file of what the report says was heard, or nothing when the
/// report cannot be read.
pub fn subrip_of(report: &str) -> Option<String> {
    let report: Report = serde_json::from_str(report).ok()?;
    let mut lines = Vec::new();
    for block in &report.transcription {
        cut(block, &mut lines);
    }
    Some(render(&lines))
}

/// Cuts one block into lines, each given the part of the block's time that its
/// share of the words comes to.
fn cut(block: &Block, lines: &mut Vec<Line>) {
    let words: Vec<&str> = block.text.split_whitespace().collect();
    let total: usize = words.iter().map(|word| word.len() + 1).sum();
    if words.is_empty() || block.offsets.to < block.offsets.from {
        return;
    }
    let duration = block.offsets.to - block.offsets.from;
    let longest = LONGEST_LINE.min(((total as u64 * LONGEST_SHOWN_MS / duration.max(1)) as usize).max(SHORTEST_LINE));

    let mut start = 0;
    let mut said = 0;
    while start < words.len() {
        let end = line_end(&words[start..], longest) + start;
        let length: usize = words[start..end].iter().map(|word| word.len() + 1).sum();
        let from = block.offsets.from + duration * said as u64 / total as u64;
        said += length;
        let to = block.offsets.from + duration * said as u64 / total as u64;
        lines.push(Line { from, to, text: words[start..end].join(" ") });
        start = end;
    }
}

/// How many of these words make the first line: up to a full stop, and no
/// further than `longest` characters, cut at a comma if there is one worth it.
fn line_end(words: &[&str], longest: usize) -> usize {
    let mut length = 0;
    let mut comma = None;
    for (position, word) in words.iter().enumerate() {
        if length > 0 && length + 1 + word.len() > longest {
            return comma.unwrap_or(position);
        }
        length += if length == 0 { word.len() } else { 1 + word.len() };
        if ends_a_sentence(word) {
            return position + 1;
        }
        if word.ends_with(',') && length >= SHORTEST_BEFORE_COMMA {
            comma = Some(position + 1);
        }
    }
    words.len()
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

    /// A report of blocks, each given as its text and when it starts and ends.
    fn report(blocks: &[(&str, u64, u64)]) -> String {
        let entries: Vec<_> = blocks
            .iter()
            .map(|(text, from, to)| serde_json::json!({"text": format!(" {text}"), "offsets": {"from": from, "to": to}}))
            .collect();
        serde_json::json!({"result": {"language": "en"}, "transcription": entries}).to_string()
    }

    fn texts(subrip: &str) -> Vec<&str> {
        subrip
            .lines()
            .filter(|line| !line.is_empty() && !line.contains(" --> ") && line.parse::<u32>().is_err())
            .collect()
    }

    #[test]
    fn a_block_of_two_sentences_is_two_lines_sharing_its_time_by_length() {
        // The case of a dialogue: nothing between the two voices but a full stop.
        let written = subrip_of(&report(&[("Where were you? At home.", 10_000, 14_000)])).expect("a report");
        assert_eq!(texts(&written), ["Where were you?", "At home."]);
        assert!(written.contains("00:00:10,000 --> 00:00:12,560"), "{written}");
        assert!(written.contains("00:00:12,560 --> 00:00:14,000"), "{written}");
    }

    #[test]
    fn lines_never_leave_the_block_they_come_from() {
        let blocks = [("One two three. Four five six. Seven eight nine.", 5_000, 12_000), ("Ten eleven.", 20_000, 23_000)];
        let written = subrip_of(&report(&blocks)).expect("a report");
        let times: Vec<&str> = written.lines().filter(|line| line.contains(" --> ")).collect();
        assert_eq!(times.len(), 4);
        assert!(times[0].starts_with("00:00:05,000"), "{times:?}");
        assert!(times[2].starts_with("00:00:09,"), "{times:?}");
        assert!(times[3].starts_with("00:00:20,000") && times[3].ends_with("00:00:23,000"), "{times:?}");
    }

    #[test]
    fn a_sentence_that_runs_on_is_cut_at_a_comma_rather_than_in_the_middle_of_a_phrase() {
        let text = "this is a rather long beginning to the sentence, and then an equally long ending that keeps going on";
        let written = subrip_of(&report(&[(text, 0, 7_000)])).expect("a report");
        let lines = texts(&written);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("sentence,"), "{lines:?}");
        assert!(lines.iter().all(|line| line.len() <= LONGEST_LINE));
    }

    #[test]
    fn a_slow_block_is_cut_so_that_no_line_stays_on_too_long() {
        let text = "slowly slowly slowly slowly slowly slowly slowly slowly slowly slowly slowly slowly";
        let written = subrip_of(&report(&[(text, 0, 24_000)])).expect("a report");
        assert!(texts(&written).len() >= 3, "{written}");
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
        let written = subrip_of(&report(&[("He said \"go.\" Then left.", 0, 4_000)])).expect("a report");
        assert_eq!(texts(&written), ["He said \"go.\"", "Then left."]);
    }

    #[test]
    fn blocks_with_nothing_said_or_backwards_times_are_left_out() {
        assert_eq!(subrip_of(&report(&[("", 0, 300)])).as_deref(), Some(""));
        assert_eq!(subrip_of(&report(&[("Hello.", 900, 100)])).as_deref(), Some(""));
        assert_eq!(subrip_of(r#"{"result":{"language":"en"}}"#).as_deref(), Some(""));
    }

    #[test]
    fn a_block_that_takes_no_time_still_gives_its_words() {
        let written = subrip_of(&report(&[("Hello there.", 4_000, 4_000)])).expect("a report");
        assert_eq!(texts(&written), ["Hello there."]);
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
