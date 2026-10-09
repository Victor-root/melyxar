//! The words of a song, as the speech tool heard them, each with the moment
//! it starts.
//!
//! Asked to cut its report at every word, the tool writes one block for each,
//! with the times it worked out for it. Those times are the tool's best guess
//! and wander over music: they are read here as they are, and it is for
//! whoever uses them to compare them with something else before believing them.

use crate::spoken_lines::Report;

/// One word heard, and when it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeardWord {
    pub text: String,
    pub from_ms: i64,
}

/// The words of the report, in the order they were heard, or nothing when the
/// report cannot be read. A block with no letters in it, like the note the tool
/// writes for music, is not a word.
pub fn words_of(report: &str) -> Option<Vec<HeardWord>> {
    let report: Report = serde_json::from_str(report).ok()?;
    Some(
        report
            .transcription
            .into_iter()
            .filter(|block| block.text.chars().any(char::is_alphanumeric))
            .map(|block| HeardWord { text: block.text.trim().to_string(), from_ms: block.offsets.from as i64 })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_block_of_the_report_is_a_word_with_the_moment_it_starts() {
        let report = r#"{"transcription":[
            {"text":" Hello","offsets":{"from":1200,"to":1500}},
            {"text":" [MUSIC]","offsets":{"from":1500,"to":3000}},
            {"text":" ♪","offsets":{"from":3000,"to":3200}},
            {"text":" world","offsets":{"from":3200,"to":3900}}]}"#;
        let words = words_of(report).expect("a report");
        assert_eq!(
            words,
            vec![
                HeardWord { text: "Hello".into(), from_ms: 1_200 },
                HeardWord { text: "[MUSIC]".into(), from_ms: 1_500 },
                HeardWord { text: "world".into(), from_ms: 3_200 },
            ]
        );
    }

    #[test]
    fn a_report_that_cannot_be_read_gives_nothing() {
        assert_eq!(words_of("not a report"), None);
        assert_eq!(words_of(r#"{"transcription":[]}"#), Some(Vec::new()));
    }
}
