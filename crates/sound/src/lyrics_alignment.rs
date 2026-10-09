//! Lining the stamps of lyrics up with where the voice starts in the song.
//!
//! The stamps of a file of lyrics say when each line starts being sung, and
//! those of a file made on another version of the song, or timed by ear, are
//! often off by a fraction of a second, the same fraction for every line or a
//! little different for each. What the song says of where a voice starts is
//! `voice_onsets`. Two things are looked for in it.
//!
//! First one shift for the whole song: the one that puts the most of the stamps
//! on the most of the starts. It is looked for over a few seconds either way,
//! a hundredth at a time, and believed only when it stands well clear of every
//! other shift and of what chance gives: a song whose voice cannot be told from
//! its instruments has no shift that does, and nothing is moved.
//!
//! Then, with that shift made, each line on its own: moved to the start of the
//! voice near it when there is one start that stands clear of the others near
//! it, and left where it is when there is none or two.
//!
//! Nothing here reads a file or launches anything: starts and stamps in,
//! shifts out.

use crate::voice::ONSETS_A_SECOND;

/// One hundredth of a second, in milliseconds.
const STEP_MS: i64 = 1_000 / ONSETS_A_SECOND as i64;

/// How far the whole song may be shifted either way, in milliseconds.
const MOST_SHIFT_MS: i64 = 4_000;
/// How far a line may be moved on its own, beyond the shift of the song.
const MOST_MOVE_MS: i64 = 350;
/// How far from the best shift another counts as a different one.
const APART_MS: i64 = 300;
/// How wide a start is taken to be, in hundredths of a second either way: a
/// stamp a little off its start still counts for it.
const SPREAD: usize = 4;
/// Fewer lines than this say nothing of where the song stands.
const FEWEST_LINES: usize = 6;
/// How far the best shift must stand over what chance gives, in the spread of
/// what chance gives.
const CLEAR_OF_CHANCE: f32 = 5.0;
/// How far the best shift must stand over the best of the shifts that are not
/// near it, as a share of how far it stands over the mean.
const CLEAR_OF_OTHERS: f32 = 0.2;
/// A shift of the song smaller than this is no shift, in milliseconds.
const NO_SHIFT_MS: i64 = 40;
/// How much stronger a start must be than the next strongest near the line to
/// be taken for the line's own.
const CLEAR_OF_NEIGHBOURS: f32 = 1.25;
/// The least lines are kept apart after being moved, in milliseconds.
const KEPT_APART_MS: i64 = 120;
/// A line moves only for a start this high in the song's own starts: the share
/// of them that are weaker.
const STRONG_START_SHARE: f32 = 0.9;

/// One line of lyrics, as stamped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub at_ms: i64,
    /// Whether anything is sung from here: a line with nothing in it marks
    /// where the singing stops, which is not where a voice starts.
    pub sung: bool,
}

/// What came of lining the lyrics up.
#[derive(Debug, Clone, PartialEq)]
pub struct Alignment {
    pub verdict: Verdict,
    /// The shift of the whole song, in milliseconds, nought unless believed.
    pub shift_ms: i64,
    /// How sure the shift is, from nought to one.
    pub confidence: f32,
    /// How far the best shift stands over chance, in the spread of chance.
    pub clear_of_chance: f32,
    /// By how much each line is moved, in milliseconds: the shift of the song
    /// and what the line's own start adds. All nought unless believed.
    pub moves_ms: Vec<i64>,
    /// How many lines are moved by 40 milliseconds or more.
    pub lines_moved: usize,
}

/// Whether the song was lined up, and if not why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Lined up: the shift and the moves are the answer.
    Aligned,
    /// The stamps already fall where the voice starts: nothing to move.
    AlreadyFits,
    /// Too few lines to say.
    TooFewLines,
    /// No shift stands clear of chance, or clear of the others: the voice
    /// cannot be told from the rest of the sound.
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

/// Lines the stamps up with the starts.
pub fn align(starts: &[f32], stamps: &[Stamp]) -> Alignment {
    let nothing = |verdict, confidence, clear_of_chance| Alignment {
        verdict,
        shift_ms: 0,
        confidence,
        clear_of_chance,
        moves_ms: vec![0; stamps.len()],
        lines_moved: 0,
    };
    let sung: Vec<i64> = stamps.iter().filter(|stamp| stamp.sung).map(|stamp| stamp.at_ms).collect();
    if sung.len() < FEWEST_LINES || starts.len() < 2 * SPREAD + 1 {
        return nothing(Verdict::TooFewLines, 0.0, 0.0);
    }

    let curve = smoothed(starts);
    let shifts: Vec<i64> = (-MOST_SHIFT_MS / STEP_MS..=MOST_SHIFT_MS / STEP_MS).map(|step| step * STEP_MS).collect();
    let scores: Vec<f32> = shifts.iter().map(|shift| score(&curve, &sung, *shift)).collect();
    let (best, _) = scores
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .expect("there are shifts to try");
    let best_score = scores[best];
    let mean = scores.iter().sum::<f32>() / scores.len() as f32;
    let spread = (scores.iter().map(|score| (score - mean).powi(2)).sum::<f32>() / scores.len() as f32).sqrt();
    let clear_of_chance = if spread > 0.0 { (best_score - mean) / spread } else { 0.0 };
    let best_of_others = shifts
        .iter()
        .zip(&scores)
        .filter(|(shift, _)| (**shift - shifts[best]).abs() >= APART_MS)
        .map(|(_, score)| *score)
        .fold(f32::MIN, f32::max);
    let clear_of_others = if best_score > mean { (best_score - best_of_others) / (best_score - mean) } else { 0.0 };
    let confidence = (clear_of_others.clamp(0.0, 1.0) * (clear_of_chance / (2.0 * CLEAR_OF_CHANCE)).clamp(0.0, 1.0))
        .clamp(0.0, 1.0);

    if clear_of_chance < CLEAR_OF_CHANCE || clear_of_others < CLEAR_OF_OTHERS {
        return nothing(Verdict::NotSure, confidence, clear_of_chance);
    }

    let shift = if shifts[best].abs() < NO_SHIFT_MS { 0 } else { shifts[best] };
    let moves = line_by_line(&curve, stamps, shift);
    let lines_moved = moves.iter().filter(|moved| moved.abs() >= NO_SHIFT_MS).count();
    let verdict = if lines_moved == 0 { Verdict::AlreadyFits } else { Verdict::Aligned };
    Alignment { verdict, shift_ms: shift, confidence, clear_of_chance, moves_ms: moves, lines_moved }
}

/// The starts spread a little either way, so that a stamp a few hundredths off
/// its start still lands on it.
fn smoothed(starts: &[f32]) -> Vec<f32> {
    let weights: Vec<f32> = (0..=SPREAD as i32)
        .map(|distance| (-(distance as f32).powi(2) / (2.0 * (SPREAD as f32 / 2.0).powi(2))).exp())
        .collect();
    (0..starts.len())
        .map(|at| {
            let mut total = 0.0;
            for (distance, weight) in weights.iter().enumerate() {
                let before = at.checked_sub(distance).map_or(0.0, |index| starts[index]);
                let after = starts.get(at + distance).copied().unwrap_or(0.0);
                total += weight * if distance == 0 { before } else { before + after };
            }
            total
        })
        .collect()
}

/// How much of the voice's starting the stamps catch when shifted by this.
fn score(curve: &[f32], stamps_ms: &[i64], shift_ms: i64) -> f32 {
    let mut total = 0.0;
    let mut counted = 0;
    for stamp in stamps_ms {
        let at = (stamp + shift_ms) / STEP_MS;
        if (0..curve.len() as i64).contains(&at) {
            total += curve[at as usize];
            counted += 1;
        }
    }
    if counted == 0 { 0.0 } else { total / counted as f32 }
}

/// Where each line goes once the song is shifted: to the start of the voice
/// near it when one stands clear, else where the shift puts it.
fn line_by_line(curve: &[f32], stamps: &[Stamp], shift_ms: i64) -> Vec<i64> {
    let strong = {
        let mut sorted: Vec<f32> = curve.to_vec();
        sorted.sort_by(f32::total_cmp);
        sorted[((sorted.len() - 1) as f32 * STRONG_START_SHARE) as usize]
    };
    let reach = (MOST_MOVE_MS / STEP_MS) as usize;
    let mut moves: Vec<i64> = stamps
        .iter()
        .map(|stamp| {
            if !stamp.sung {
                return shift_ms;
            }
            let centre = (stamp.at_ms + shift_ms) / STEP_MS;
            if centre < 0 || centre as usize >= curve.len() {
                return shift_ms;
            }
            let centre = centre as usize;
            let from = centre.saturating_sub(reach);
            let to = (centre + reach + 1).min(curve.len());
            let Some(peak) = (from..to).max_by(|a, b| curve[*a].total_cmp(&curve[*b])) else {
                return shift_ms;
            };
            let runner_up = (from..to)
                .filter(|at| at.abs_diff(peak) > SPREAD * 2)
                .map(|at| curve[at])
                .fold(0.0, f32::max);
            if curve[peak] >= strong && curve[peak] >= CLEAR_OF_NEIGHBOURS * runner_up {
                (peak as i64 - stamp.at_ms / STEP_MS) * STEP_MS
            } else {
                shift_ms
            }
        })
        .collect();
    keep_in_order(stamps, &mut moves, shift_ms);
    moves
}

/// A line moved past the one before it, or onto it, goes back to where the
/// shift of the song puts it.
fn keep_in_order(stamps: &[Stamp], moves: &mut [i64], shift_ms: i64) {
    let mut last: Option<i64> = None;
    for (stamp, moved) in stamps.iter().zip(moves.iter_mut()) {
        let mut at = stamp.at_ms + *moved;
        if last.is_some_and(|before| at < before + KEPT_APART_MS && stamp.at_ms >= before + KEPT_APART_MS) {
            *moved = shift_ms;
            at = stamp.at_ms + shift_ms;
        }
        last = Some(at.max(last.unwrap_or(i64::MIN)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{VOICE_SAMPLES_A_SECOND, tests::{a_song, a_song_with_guitar}, voice_onsets};

    fn stamps_of(starts: &[f32], off_by_ms: i64) -> Vec<Stamp> {
        starts.iter().map(|start| Stamp { at_ms: (start * 1000.0) as i64 + off_by_ms, sung: true }).collect()
    }

    const STARTS: [f32; 12] = [2.0, 4.6, 8.1, 9.7, 13.4, 17.9, 19.2, 24.3, 27.8, 29.1, 33.0, 36.2];

    fn song() -> Vec<f32> {
        voice_onsets(&a_song(40.0, &STARTS, 1.1))
    }

    #[test]
    fn a_song_stamped_late_is_taken_back_to_where_the_voice_starts() {
        let onsets = song();
        let stamped = stamps_of(&STARTS, 350);
        let found = align(&onsets, &stamped);
        assert_eq!(found.verdict, Verdict::Aligned, "{found:?}");
        assert!((found.shift_ms + 350).abs() < 30, "shift {}", found.shift_ms);
        for (moved, (stamp, start)) in found.moves_ms.iter().zip(stamped.iter().zip(STARTS)) {
            let lands = stamp.at_ms + moved;
            assert!((lands - (start * 1000.0) as i64).abs() < 30, "a line lands at {lands} for a start at {start}");
        }
        assert!(found.confidence > 0.3, "{}", found.confidence);
    }

    #[test]
    fn stamps_off_by_a_little_each_are_each_moved_to_their_own_start() {
        let onsets = song();
        let jitter = [120, -150, 90, -60, 200, -110, 40, -180, 130, -90, 160, -70];
        let stamped: Vec<Stamp> =
            STARTS.iter().zip(jitter).map(|(start, off)| Stamp { at_ms: (start * 1000.0) as i64 + off, sung: true }).collect();
        let found = align(&onsets, &stamped);
        assert!(matches!(found.verdict, Verdict::Aligned | Verdict::AlreadyFits), "{found:?}");
        let near = found
            .moves_ms
            .iter()
            .zip(stamped.iter().zip(STARTS))
            .filter(|(moved, (stamp, start))| (stamp.at_ms + **moved - (start * 1000.0) as i64).abs() < 30)
            .count();
        assert!(near >= 10, "{near} of 12 lines land on their start: {found:?}");
    }

    #[test]
    fn a_guitar_starting_notes_of_its_own_among_the_voice_does_not_hide_the_shift() {
        let guitar = [1.1, 3.3, 6.2, 7.0, 10.9, 12.2, 15.6, 16.4, 21.7, 22.5, 25.1, 26.9, 30.4, 31.6, 34.2, 38.0];
        let onsets = voice_onsets(&a_song_with_guitar(40.0, &STARTS, &guitar, 0.9));
        let found = align(&onsets, &stamps_of(&STARTS, -280));
        assert_eq!(found.verdict, Verdict::Aligned, "{found:?}");
        assert!((found.shift_ms - 280).abs() < 40, "shift {}", found.shift_ms);
    }

    #[test]
    fn stamps_that_already_fit_are_left_where_they_are() {
        let onsets = song();
        let found = align(&onsets, &stamps_of(&STARTS, 20));
        assert!(matches!(found.verdict, Verdict::AlreadyFits | Verdict::Aligned), "{found:?}");
        assert!(found.shift_ms.abs() < 40, "{}", found.shift_ms);
        assert!(found.moves_ms.iter().all(|moved| moved.abs() <= 120), "{:?}", found.moves_ms);
    }

    #[test]
    fn a_song_without_a_voice_cannot_be_lined_up_and_nothing_is_moved() {
        let onsets = voice_onsets(&a_song(40.0, &[], 1.0));
        let found = align(&onsets, &stamps_of(&STARTS, 350));
        assert_eq!(found.verdict, Verdict::NotSure, "{found:?}");
        assert_eq!(found.shift_ms, 0);
        assert!(found.moves_ms.iter().all(|moved| *moved == 0));
    }

    #[test]
    fn too_few_lines_say_nothing_and_a_line_with_nothing_sung_is_not_counted() {
        let onsets = song();
        assert_eq!(align(&onsets, &stamps_of(&STARTS[..3], 0)).verdict, Verdict::TooFewLines);
        let mut empty: Vec<Stamp> = stamps_of(&STARTS[..4], 0);
        empty.extend((0..4).map(|n| Stamp { at_ms: 40_000 + n * 1_000, sung: false }));
        assert_eq!(align(&onsets, &empty).verdict, Verdict::TooFewLines);
    }

    #[test]
    fn the_rate_the_voice_is_read_at_is_the_rate_it_is_made_for() {
        assert_eq!(VOICE_SAMPLES_A_SECOND, 8_000);
        assert_eq!(STEP_MS, 10);
    }
}
