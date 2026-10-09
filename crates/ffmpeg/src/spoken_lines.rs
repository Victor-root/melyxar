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
/// A line is shown no longer than this plus the time to say its characters
/// slowly: a block often runs on over music or silence after a few words,
/// and the words would stay up over nothing being said.
const SHOWN_AT_MOST_BASE_MS: u64 = 1_500;
const SHOWN_AT_MOST_PER_CHARACTER_MS: u64 = 120;
/// How many blocks in a row repeating the ones before them make a loop.
const SHORTEST_LOOP: usize = 6;

#[derive(Deserialize)]
pub(crate) struct Report {
    #[serde(default)]
    pub(crate) transcription: Vec<Block>,
}

#[derive(Deserialize)]
pub(crate) struct Block {
    pub(crate) text: String,
    pub(crate) offsets: Offsets,
}

#[derive(Deserialize)]
pub(crate) struct Offsets {
    pub(crate) from: u64,
    pub(crate) to: u64,
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

/// Where the tool got stuck repeating itself: a stretch of blocks each saying
/// what the block two before it said, which is how a single phrase or a pair
/// of them goes round and round until the end of the video.
#[derive(Debug, PartialEq, Eq)]
pub struct Loop {
    pub from_ms: u64,
    pub to_ms: u64,
    pub blocks: usize,
    pub text: String,
}

/// The first loop in the report, or nothing when there is none or the report
/// cannot be read.
pub fn loop_in(report: &str) -> Option<Loop> {
    let report: Report = serde_json::from_str(report).ok()?;
    let blocks = &report.transcription;
    let repeats = |at: usize| blocks[at].text.trim() == blocks[at - 2].text.trim();
    let mut at = 2;
    while at < blocks.len() {
        if !repeats(at) {
            at += 1;
            continue;
        }
        let start = at - 2;
        while at < blocks.len() && repeats(at) {
            at += 1;
        }
        if at - start >= SHORTEST_LOOP {
            return Some(Loop {
                from_ms: blocks[start].offsets.from,
                to_ms: blocks[at - 1].offsets.to,
                blocks: at - start,
                text: blocks[start].text.trim().to_string(),
            });
        }
    }
    None
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
        let shared_to = block.offsets.from + duration * said as u64 / total as u64;
        let text = words[start..end].join(" ");
        let at_most = SHOWN_AT_MOST_BASE_MS + SHOWN_AT_MOST_PER_CHARACTER_MS * text.chars().count() as u64;
        lines.push(Line { from, to: shared_to.min(from + at_most), text });
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
        let blocks = [("One two three. Four five six. Seven eight nine.", 5_000, 12_000), ("Ten eleven.", 20_000, 22_000)];
        let written = subrip_of(&report(&blocks)).expect("a report");
        let times: Vec<&str> = written.lines().filter(|line| line.contains(" --> ")).collect();
        assert_eq!(times.len(), 4);
        assert!(times[0].starts_with("00:00:05,000"), "{times:?}");
        assert!(times[2].starts_with("00:00:09,"), "{times:?}");
        assert!(times[3].starts_with("00:00:20,000") && times[3].ends_with("00:00:22,000"), "{times:?}");
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
    fn a_few_words_in_a_block_that_runs_on_do_not_stay_up_over_the_silence() {
        let written = subrip_of(&report(&[("All right, get out.", 932_000, 956_000)])).expect("a report");
        assert!(written.contains("00:15:32,000 --> 00:15:35,780"), "{written}");
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
    fn a_pair_of_phrases_going_round_is_a_loop_from_its_first_block() {
        let mut blocks = vec![("Fine.", 0, 2_000), ("Then go.", 2_000, 4_000)];
        for turn in 0..10u64 {
            let text = if turn % 2 == 0 { "I am so awkwardly." } else { "You and I should get it." };
            blocks.push((text, 5_000 + turn * 1_000, 6_000 + turn * 1_000));
        }
        let found = loop_in(&report(&blocks)).expect("a loop");
        assert_eq!((found.from_ms, found.to_ms, found.blocks), (5_000, 15_000, 10));
        assert_eq!(found.text, "I am so awkwardly.");
    }

    #[test]
    fn one_phrase_said_again_and_again_is_a_loop_too() {
        let blocks: Vec<_> = (0..8u64).map(|turn| ("Thank you.", turn * 1_000, turn * 1_000 + 900)).collect();
        assert_eq!(loop_in(&report(&blocks)).map(|found| found.blocks), Some(8));
    }

    #[test]
    fn a_phrase_said_a_few_times_in_a_dialogue_is_not_a_loop() {
        let blocks = [
            ("No.", 0, 500),
            ("No.", 600, 1_100),
            ("No.", 1_200, 1_700),
            ("Yes.", 2_000, 2_500),
            ("Why?", 3_000, 3_500),
        ];
        assert_eq!(loop_in(&report(&blocks)), None);
        assert_eq!(loop_in(&report(&[])), None);
        assert_eq!(loop_in("not a report"), None);
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
