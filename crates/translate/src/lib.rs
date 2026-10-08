//! Subtitles turned from English into French.
//!
//! Everything that is a question of text lives here and is pure: reading and
//! writing a subtitle file, cutting a line into the sentences a translation
//! model handles well, and putting the answers back where the questions came
//! from. The model itself is behind `Engine`, which is handed lines and gives
//! lines back.
//!
//! Nothing here depends on another crate of Melyxar, and nothing here knows
//! about a database, a job or a download.

mod engine;
mod sentences;
mod subrip;

pub use engine::{Engine, Error};
pub use sentences::sentences;
pub use subrip::{Cue, parse, render};

/// What the listening writes around a line that is sung rather than said.
const NOTES: [char; 3] = ['♪', '♫', '♬'];

/// Whether a cue is music: the notes are what says so, in the language it was
/// heard in.
fn is_sung(text: &str) -> bool {
    text.contains(NOTES)
}

/// How many sentences are handed to the model at once. A model reads a batch
/// together, so more is faster, up to the point where a long film's worth of
/// memory is held for nothing.
const IN_ONE_BATCH: usize = 32;

/// Translates every cue of a subtitle, keeping when each one is shown.
///
/// A cue is cut into sentences first: a model given two sentences in one go
/// is known to answer only the first and quietly drop the rest, which in a
/// subtitle is a line of speech nobody can read. The answers are put back
/// together in the cue they came from. `translate` is asked for each batch of
/// sentences and must answer as many lines as it was given.
///
/// A sung cue is kept as it was heard, notes and words: the model does not
/// know a note, drops it or leaves nothing at all, and what it makes of lyrics
/// is worse than the words that were sung.
pub fn translate_cues<E>(
    cues: &[Cue],
    mut translate: impl FnMut(&[String]) -> Result<Vec<String>, E>,
) -> Result<Vec<Cue>, E> {
    let cut: Vec<Vec<String>> = cues
        .iter()
        .map(|cue| if is_sung(&cue.text) { Vec::new() } else { sentences(&cue.text) })
        .collect();
    let every: Vec<String> = cut.iter().flatten().cloned().collect();

    let mut answered = Vec::with_capacity(every.len());
    for batch in every.chunks(IN_ONE_BATCH) {
        answered.extend(translate(batch)?);
    }

    let mut answers = answered.into_iter();
    Ok(cues
        .iter()
        .zip(&cut)
        .map(|(cue, own)| Cue {
            timing: cue.timing.clone(),
            text: if is_sung(&cue.text) {
                cue.text.clone()
            } else {
                own.iter()
                    .filter_map(|_| answers.next())
                    .map(|line| line.trim().to_string())
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            },
        })
        .filter(|cue| !cue.text.is_empty())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(timing: &str, text: &str) -> Cue {
        Cue { timing: timing.to_string(), text: text.to_string() }
    }

    /// A model that shouts what it was asked and counts the batches.
    fn shouting(batches: &mut Vec<usize>) -> impl FnMut(&[String]) -> Result<Vec<String>, ()> + '_ {
        move |lines| {
            batches.push(lines.len());
            Ok(lines.iter().map(|line| line.to_uppercase()).collect())
        }
    }

    #[test]
    fn each_cue_keeps_its_timing_and_gets_all_of_its_sentences_back() {
        let cues = [
            cue("00:00:00,000 --> 00:00:04,000", "Wait, hold on a second. Who left the door open?"),
            cue("00:00:04,000 --> 00:00:06,000", "Nobody did."),
        ];
        let mut batches = Vec::new();
        let done = translate_cues(&cues, shouting(&mut batches)).expect("translated");
        assert_eq!(done.len(), 2);
        assert_eq!(done[0].timing, "00:00:00,000 --> 00:00:04,000");
        assert_eq!(done[0].text, "WAIT, HOLD ON A SECOND. WHO LEFT THE DOOR OPEN?");
        assert_eq!(done[1].text, "NOBODY DID.");
        assert_eq!(batches, [3], "the three sentences went in together");
    }

    #[test]
    fn a_long_film_goes_through_in_batches_and_comes_back_in_order() {
        let cues: Vec<Cue> = (0..100)
            .map(|number| cue(&format!("00:00:{number:02},000 --> 00:00:{number:02},500"), &format!("Line {number}.")))
            .collect();
        let mut batches = Vec::new();
        let done = translate_cues(&cues, shouting(&mut batches)).expect("translated");
        assert_eq!(batches, [32, 32, 32, 4]);
        assert_eq!(done.len(), 100);
        assert_eq!(done[57].text, "LINE 57.");
    }

    #[test]
    fn a_cue_the_model_leaves_empty_is_left_out_rather_than_shown_blank() {
        let cues = [cue("t1", "Hmm."), cue("t2", "Fine.")];
        let done = translate_cues(&cues, |lines: &[String]| -> Result<_, ()> {
            Ok(lines.iter().map(|line| if line == "Hmm." { String::new() } else { line.clone() }).collect())
        })
        .expect("translated");
        assert_eq!(done, [cue("t2", "Fine.")]);
    }

    #[test]
    fn a_sung_cue_is_kept_as_heard_and_never_sent_to_the_model() {
        let cues = [
            cue("t1", "♪ Cat with their ills ♪"),
            cue("t2", "♪"),
            cue("t3", "Who is there?"),
            cue("t4", "♫ la la la"),
        ];
        let mut batches = Vec::new();
        let done = translate_cues(&cues, shouting(&mut batches)).expect("translated");
        assert_eq!(batches, [1], "only the spoken sentence went in");
        assert_eq!(
            done,
            [
                cue("t1", "♪ Cat with their ills ♪"),
                cue("t2", "♪"),
                cue("t3", "WHO IS THERE?"),
                cue("t4", "♫ la la la"),
            ]
        );
    }

    #[test]
    fn a_failure_of_the_model_is_the_failure_of_the_whole_subtitle() {
        let cues = [cue("t", "Hello.")];
        assert_eq!(translate_cues(&cues, |_: &[String]| Err::<Vec<String>, _>("broken")), Err("broken"));
    }
}
