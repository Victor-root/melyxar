//! Lining the stamps of lyrics up with the words heard in the song.
//!
//! The stamps of a file of lyrics say when each line starts being sung, and
//! those of a file made on another version of the song, or timed by ear, are
//! often off by a fraction of a second or by a few seconds, the same for every
//! line or a little different for each. The speech tool, listening to the
//! song, writes down the words it hears and when. It is not asked what is
//! sung: the words are known. What it heard is used only where it agrees with
//! them, and whatever it makes up on top of the music finds no line to agree
//! with and is left aside.
//!
//! The words heard are laid beside the words of the lyrics in the order of
//! both, each heard word taking the lyric word it spells, if any. Each line
//! that was heard at some of its words learns from them where it starts: the
//! moment of a word less the time the words before it take. A line whose
//! answer is far from those of the lines around it is a word taken for
//! another one, a chorus heard twice, and is not believed. The others give
//! their neighbours the move to make, smoothed over a handful of them, since
//! the speech tool's moments wander by a few tenths of a second on music.
//!
//! Nothing here reads a file or launches anything: words and stamps in, moves
//! out.

use melyxar_core::text::fold_accents;

/// Fewer sung lines than this say nothing of where the song stands.
const FEWEST_LINES: usize = 6;
/// Fewer lines heard than this are too few to believe.
const FEWEST_HEARD: usize = 4;
/// The share of the sung lines that must have been heard.
const SMALLEST_SHARE: f32 = 0.25;
/// How many of the lines heard on each side of a line say whether its answer
/// is believed.
const SAY_BY: usize = 3;
/// How far from what the lines around it say an answer may be and be believed,
/// in milliseconds.
const BELIEVED_WITHIN_MS: i64 = 2_500;
/// How many lines heard give a line its move: the nearest ones.
const MOVE_FROM: usize = 5;
/// A move smaller than this is no move: the speech tool is not that precise.
const LEAST_MOVE_MS: i64 = 250;
/// The least lines are kept apart after being moved, in milliseconds.
const KEPT_APART_MS: i64 = 100;
/// What a word takes to be sung, at the least and at the most, in milliseconds.
const SHORTEST_WORD_MS: i64 = 150;
const LONGEST_WORD_MS: i64 = 600;
/// The pace of the last line, which has no line after it to be read against.
const LAST_LINE_WORD_MS: i64 = 400;
/// The longest a line is taken to last.
const LONGEST_LINE_MS: i64 = 6_000;

/// One line of lyrics, as stamped. A line with nothing in it marks where the
/// singing stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricLine {
    pub at_ms: i64,
    pub text: String,
}

/// One word heard in the song, and when it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeardWord {
    pub text: String,
    pub at_ms: i64,
}

/// What came of lining the lyrics up.
#[derive(Debug, Clone, PartialEq)]
pub struct Alignment {
    pub verdict: Verdict,
    /// What most of the song moves by, in milliseconds, nought unless lined up.
    pub shift_ms: i64,
    /// The share of the sung lines that were heard and believed, from nought
    /// to one.
    pub confidence: f32,
    /// By how much each line is moved, in milliseconds. All nought unless
    /// lined up.
    pub moves_ms: Vec<i64>,
    /// How many lines are moved.
    pub lines_moved: usize,
    /// How many lines were heard and believed.
    pub lines_heard: usize,
}

/// Whether the song was lined up, and if not why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Lined up: the moves are the answer.
    Aligned,
    /// The stamps already fall where the words are heard: nothing to move.
    AlreadyFits,
    /// Too few lines to say.
    TooFewLines,
    /// Too few of the lines were heard to be sure of anything.
    NotSure,
}

impl Verdict {
    pub fn as_word(self) -> &'static str {
        match self {
            Self::Aligned => "aligned",
            Self::AlreadyFits => "already fits",
            Self::TooFewLines => "too few lines",
            Self::NotSure => "not sure",
        }
    }
}

/// Lines the stamps up with the words heard.
pub fn align(lines: &[LyricLine], heard: &[HeardWord]) -> Alignment {
    let nothing = |verdict, confidence, lines_heard| Alignment {
        verdict,
        shift_ms: 0,
        confidence,
        moves_ms: vec![0; lines.len()],
        lines_moved: 0,
        lines_heard,
    };
    let words: Vec<Vec<String>> = lines.iter().map(|line| words_of(&line.text)).collect();
    let sung = words.iter().filter(|words| !words.is_empty()).count();
    if sung < FEWEST_LINES {
        return nothing(Verdict::TooFewLines, 0.0, 0);
    }

    let answers = what_each_line_says(lines, &words, heard);
    let believed = believed_among(&answers);
    let share = believed.len() as f32 / sung as f32;
    if believed.len() < FEWEST_HEARD || share < SMALLEST_SHARE {
        return nothing(Verdict::NotSure, share, believed.len());
    }

    let mut moves: Vec<i64> = (0..lines.len()).map(|line| move_of(line, &believed)).collect();
    keep_in_order(lines, &mut moves);
    let lines_moved = moves.iter().filter(|moved| **moved != 0).count();
    let shift_ms = tenth_of_a_second(median(believed.iter().map(|(_, answer)| *answer).collect()));
    let verdict = if lines_moved == 0 { Verdict::AlreadyFits } else { Verdict::Aligned };
    Alignment { verdict, shift_ms, confidence: share.min(1.0), moves_ms: moves, lines_moved, lines_heard: believed.len() }
}

/// The words of a text, in lower case and without accents or punctuation.
fn words_of(text: &str) -> Vec<String> {
    fold_accents(text)
        .chars()
        .filter(|letter| !matches!(letter, '\'' | '’'))
        .flat_map(char::to_lowercase)
        .map(|letter| if letter.is_alphanumeric() { letter } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// How much finding a word is worth: the short words every song is full of
/// say little of where in it we are.
fn weight(word: &str) -> f32 {
    match word.chars().count() {
        0..=2 => 0.3,
        3 => 0.6,
        _ => 1.0,
    }
}

/// Whether two words are the same word: spelled alike, or a letter or two
/// apart for a long one, since the speech tool misspells what it hears.
fn same_word(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let longest = a.chars().count().max(b.chars().count());
    let allowed = match longest {
        0..=3 => 0,
        4..=6 => 1,
        _ => 2,
    };
    allowed > 0 && a.chars().count().abs_diff(b.chars().count()) <= allowed && edits(a, b) <= allowed
}

/// How many letters have to be changed, added or taken out to turn one word
/// into the other.
fn edits(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut before: Vec<usize> = (0..=b.len()).collect();
    for (row, letter) in a.iter().enumerate() {
        let mut current = vec![row + 1];
        for (column, other) in b.iter().enumerate() {
            let changed = before[column] + usize::from(letter != other);
            current.push(changed.min(before[column + 1] + 1).min(current[column] + 1));
        }
        before = current;
    }
    before[b.len()]
}

/// Where each line says it starts, according to the words heard of it, minus
/// where it is stamped: how far it is from where it should be. Nothing for a
/// line none of whose words was heard.
fn what_each_line_says(lines: &[LyricLine], words: &[Vec<String>], heard: &[HeardWord]) -> Vec<Option<i64>> {
    let flat: Vec<(usize, usize, &str)> = words
        .iter()
        .enumerate()
        .flat_map(|(line, words)| words.iter().enumerate().map(move |(position, word)| (line, position, word.as_str())))
        .collect();
    let listened: Vec<(i64, String)> = heard
        .iter()
        .flat_map(|word| words_of(&word.text).into_iter().map(move |spelled| (word.at_ms, spelled)))
        .collect();

    let mut heard_of_line: Vec<Vec<(i64, i64)>> = vec![Vec::new(); lines.len()];
    for (wanted, found) in matched(&flat, &listened) {
        let (line, position, _) = flat[wanted];
        heard_of_line[line].push((position as i64, listened[found].0));
    }
    heard_of_line
        .into_iter()
        .zip(lines)
        .enumerate()
        .map(|(at, (heard, line))| start_of(&heard, pace_of(lines, &words[at], at)).map(|start| start - line.at_ms))
        .collect()
}

/// Where a line starts, from the words of it that were heard, as their
/// position in the line and their moment: the first one heard less the time
/// the words before it take, which the words heard say if there are two, and
/// the length of the line says if there is one.
fn start_of(heard: &[(i64, i64)], pace_of_the_line: i64) -> Option<i64> {
    let (first_position, first_moment) = *heard.first()?;
    let (last_position, last_moment) = *heard.last()?;
    let pace = if last_position > first_position {
        ((last_moment - first_moment) / (last_position - first_position)).clamp(SHORTEST_WORD_MS, LONGEST_WORD_MS)
    } else {
        pace_of_the_line
    };
    Some(first_moment - first_position * pace)
}

/// How long a word of this line takes to sing, by how long the line lasts.
fn pace_of(lines: &[LyricLine], words: &[String], line: usize) -> i64 {
    match lines.get(line + 1) {
        Some(next) => {
            let lasts = (next.at_ms - lines[line].at_ms).clamp(1, LONGEST_LINE_MS);
            (lasts / words.len().max(1) as i64).clamp(SHORTEST_WORD_MS, LONGEST_WORD_MS)
        }
        None => LAST_LINE_WORD_MS,
    }
}

/// Which heard word is which word of the lyrics, as pairs of positions in the
/// order of both: the most words, the long ones counting for more, that can be
/// found in the same order on both sides.
fn matched(wanted: &[(usize, usize, &str)], heard: &[(i64, String)]) -> Vec<(usize, usize)> {
    let width = heard.len() + 1;
    let mut best = vec![0.0f32; (wanted.len() + 1) * width];
    for i in 1..=wanted.len() {
        for j in 1..=heard.len() {
            let skipped = best[(i - 1) * width + j].max(best[i * width + j - 1]);
            best[i * width + j] = if same_word(wanted[i - 1].2, &heard[j - 1].1) {
                skipped.max(best[(i - 1) * width + j - 1] + weight(wanted[i - 1].2))
            } else {
                skipped
            };
        }
    }
    let (mut i, mut j) = (wanted.len(), heard.len());
    let mut pairs = Vec::new();
    while i > 0 && j > 0 {
        let here = best[i * width + j];
        if here == best[(i - 1) * width + j] {
            i -= 1;
        } else if here == best[i * width + j - 1] {
            j -= 1;
        } else {
            pairs.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        }
    }
    pairs.reverse();
    pairs
}

/// The lines whose answer agrees with the answers of the lines heard around
/// them, with their answers.
fn believed_among(answers: &[Option<i64>]) -> Vec<(usize, i64)> {
    let heard: Vec<(usize, i64)> = answers.iter().enumerate().filter_map(|(line, answer)| answer.map(|a| (line, a))).collect();
    heard
        .iter()
        .enumerate()
        .filter(|(at, (_, answer))| {
            let around = heard[at.saturating_sub(SAY_BY)..(at + SAY_BY + 1).min(heard.len())]
                .iter()
                .map(|(_, answer)| *answer)
                .collect();
            (answer - median(around)).abs() <= BELIEVED_WITHIN_MS
        })
        .map(|(_, line)| *line)
        .collect()
}

/// By how much a line is moved: what the nearest lines that were heard say,
/// as a middle of them, and nothing when it comes to less than the speech
/// tool could tell.
fn move_of(line: usize, believed: &[(usize, i64)]) -> i64 {
    let mut nearest: Vec<&(usize, i64)> = believed.iter().collect();
    nearest.sort_by_key(|(at, _)| at.abs_diff(line));
    let moved = median(nearest.iter().take(MOVE_FROM).map(|(_, answer)| *answer).collect());
    if moved.abs() < LEAST_MOVE_MS { 0 } else { tenth_of_a_second(moved) }
}

/// A line moved before the one before it, or onto it, goes no further than
/// leaves them apart, if they were apart; and none goes before the start.
fn keep_in_order(lines: &[LyricLine], moves: &mut [i64]) {
    let mut before: Option<(i64, i64)> = None;
    for (line, moved) in lines.iter().zip(moves.iter_mut()) {
        let mut at = line.at_ms + *moved;
        if let Some((was, now)) = before
            && line.at_ms >= was + KEPT_APART_MS
            && at < now + KEPT_APART_MS
        {
            at = now + KEPT_APART_MS;
        }
        at = at.max(0);
        *moved = at - line.at_ms;
        before = Some((line.at_ms, at));
    }
}

fn median(mut values: Vec<i64>) -> i64 {
    values.sort_unstable();
    values.get(values.len() / 2).copied().unwrap_or(0)
}

fn tenth_of_a_second(milliseconds: i64) -> i64 {
    (milliseconds as f64 / 10.0).round() as i64 * 10
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORD_MS: i64 = 400;

    /// A word of seven letters that no other word of the song is within two
    /// letters of.
    fn a_word(seed: u64) -> String {
        let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (0..7)
            .map(|_| {
                state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                char::from(b'a' + ((state >> 33) % 26) as u8)
            })
            .collect()
    }

    /// A song of `count` lines of three words each, one every four seconds,
    /// each word its own.
    fn a_song(count: usize) -> Vec<LyricLine> {
        (0..count)
            .map(|n| LyricLine {
                at_ms: 5_000 + n as i64 * 4_000,
                text: (0..3).map(|k| a_word((n * 3 + k) as u64)).collect::<Vec<_>>().join(" "),
            })
            .collect()
    }

    /// The words of those lines as the speech tool hears them when they are
    /// sung `late_by` later than they are stamped.
    fn heard_late_by(lines: &[LyricLine], late_by: i64) -> Vec<HeardWord> {
        lines
            .iter()
            .flat_map(|line| {
                words_of(&line.text).into_iter().enumerate().map(move |(position, word)| HeardWord {
                    text: format!(" {word}"),
                    at_ms: line.at_ms + late_by + position as i64 * WORD_MS,
                })
            })
            .collect()
    }

    fn lined_up(alignment: &Alignment, lines: &[LyricLine], line: usize) -> i64 {
        lines[line].at_ms + alignment.moves_ms[line]
    }

    #[test]
    fn lines_stamped_early_are_moved_to_where_their_words_are_heard() {
        let lines = a_song(20);
        let alignment = align(&lines, &heard_late_by(&lines, 1_500));
        assert_eq!(alignment.verdict, Verdict::Aligned);
        assert_eq!(alignment.shift_ms, 1_500);
        assert_eq!(alignment.lines_moved, 20);
        assert_eq!(alignment.lines_heard, 20);
        assert!(alignment.confidence > 0.99);
        for line in 0..20 {
            assert_eq!(lined_up(&alignment, &lines, line), lines[line].at_ms + 1_500, "line {line}");
        }
    }

    #[test]
    fn lines_stamped_late_are_moved_back() {
        let lines = a_song(12);
        let alignment = align(&lines, &heard_late_by(&lines, -2_000));
        assert_eq!(alignment.verdict, Verdict::Aligned);
        assert_eq!(alignment.shift_ms, -2_000);
        assert_eq!(lined_up(&alignment, &lines, 3), lines[3].at_ms - 2_000);
    }

    #[test]
    fn the_wandering_of_the_speech_tool_is_not_a_reason_to_move_a_line() {
        let lines = a_song(20);
        let mut heard = heard_late_by(&lines, 0);
        for (n, word) in heard.iter_mut().enumerate() {
            word.at_ms += [120, -180, 90, -60, 200, -150][n % 6];
        }
        let alignment = align(&lines, &heard);
        assert_eq!(alignment.verdict, Verdict::AlreadyFits);
        assert_eq!(alignment.lines_moved, 0);
        assert!(alignment.moves_ms.iter().all(|moved| *moved == 0));
    }

    #[test]
    fn a_line_heard_late_in_the_middle_of_its_words_still_starts_where_its_first_word_does() {
        let lines = a_song(10);
        // Only the last two words of each line were heard.
        let heard: Vec<HeardWord> = heard_late_by(&lines, 1_000)
            .into_iter()
            .enumerate()
            .filter(|(n, _)| n % 3 != 0)
            .map(|(_, word)| word)
            .collect();
        let alignment = align(&lines, &heard);
        assert_eq!(alignment.verdict, Verdict::Aligned);
        assert_eq!(alignment.lines_heard, 10);
        assert_eq!(alignment.moves_ms[4], 1_000);
    }

    #[test]
    fn lines_nobody_heard_are_moved_with_the_lines_around_them() {
        let lines = a_song(14);
        let heard: Vec<HeardWord> = heard_late_by(&lines, 1_200)
            .into_iter()
            .filter(|word| ![6, 7].iter().any(|n| (0..3).any(|k| word.text.trim() == a_word(n * 3 + k))))
            .collect();
        let alignment = align(&lines, &heard);
        assert_eq!(alignment.verdict, Verdict::Aligned);
        assert_eq!(alignment.lines_heard, 12);
        assert_eq!(alignment.moves_ms[6], 1_200);
        assert_eq!(alignment.moves_ms[7], 1_200);
    }

    #[test]
    fn what_the_tool_makes_up_over_the_music_moves_nothing() {
        let lines = a_song(16);
        let invented = ["thank", "you", "for", "watching", "subscribe", "music", "applause", "another"];
        let heard: Vec<HeardWord> = (0..80)
            .map(|n| HeardWord { text: format!(" {}", invented[n % invented.len()]), at_ms: 4_000 + n as i64 * 700 })
            .collect();
        let alignment = align(&lines, &heard);
        assert_eq!(alignment.verdict, Verdict::NotSure);
        assert_eq!(alignment.lines_moved, 0);
        assert!(alignment.moves_ms.iter().all(|moved| *moved == 0));
    }

    #[test]
    fn a_song_nothing_was_heard_of_is_not_moved() {
        let lines = a_song(16);
        let alignment = align(&lines, &[]);
        assert_eq!(alignment.verdict, Verdict::NotSure);
        assert_eq!(alignment.lines_heard, 0);
    }

    #[test]
    fn a_line_taken_for_another_one_is_not_believed() {
        // A chorus heard twice, and one of its words taken for the other one:
        // the line says it is half a minute from where it is stamped, where
        // the lines around it say a second.
        let mut answers: Vec<Option<i64>> = vec![Some(1_000); 11];
        answers[5] = Some(31_000);
        answers[3] = None;
        let believed = believed_among(&answers);
        assert_eq!(believed.len(), 9);
        assert!(believed.iter().all(|(line, answer)| *line != 5 && *answer == 1_000));
    }

    #[test]
    fn words_are_found_through_accents_capitals_punctuation_and_a_misheard_letter() {
        assert_eq!(words_of("Où est l’été, Ça va?"), vec!["ou", "est", "lete", "ca", "va"]);
        assert!(same_word("ete", "ete"));
        assert!(same_word("tomorrow", "tomorow"));
        assert!(!same_word("cat", "cut"), "a short word is not forgiven");
        assert!(!same_word("tomorrow", "yesterday"));
    }

    #[test]
    fn too_few_sung_lines_say_nothing() {
        let lines = a_song(4);
        let alignment = align(&lines, &heard_late_by(&lines, 2_000));
        assert_eq!(alignment.verdict, Verdict::TooFewLines);
        assert_eq!(alignment.lines_moved, 0);
    }

    #[test]
    fn a_line_with_nothing_in_it_is_moved_with_the_others() {
        let mut lines = a_song(12);
        lines[5].text = String::new();
        let alignment = align(&lines, &heard_late_by(&lines, 1_000));
        assert_eq!(alignment.verdict, Verdict::Aligned);
        assert_eq!(alignment.moves_ms[5], 1_000);
        assert_eq!(alignment.lines_heard, 11);
    }

    #[test]
    fn lines_stay_in_the_order_they_were_and_none_goes_before_the_start() {
        let lines = vec![
            LyricLine { at_ms: 300, text: "first line here".into() },
            LyricLine { at_ms: 1_000, text: "second line here".into() },
            LyricLine { at_ms: 2_000, text: "third line here".into() },
        ];
        let mut moves = vec![-1_000, -2_000, -500];
        keep_in_order(&lines, &mut moves);
        let at: Vec<i64> = lines.iter().zip(&moves).map(|(line, moved)| line.at_ms + moved).collect();
        assert_eq!(at[0], 0);
        assert!(at[1] >= at[0] + KEPT_APART_MS && at[2] >= at[1] + KEPT_APART_MS, "{at:?}");
    }

    #[test]
    fn words_are_compared_by_the_edits_they_are_apart() {
        assert_eq!(edits("kitten", "sitting"), 3);
        assert_eq!(edits("same", "same"), 0);
        assert_eq!(edits("", "abc"), 3);
    }
}
