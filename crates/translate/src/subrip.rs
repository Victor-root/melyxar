//! A subtitle file in the SubRip form, which is what listening writes.

/// One line of subtitle: when it is shown, as the file writes it, and what it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    /// `00:00:01,000 --> 00:00:03,500`, kept as it was written: it is never
    /// read, only handed back.
    pub timing: String,
    pub text: String,
}

/// The cues of a file, in the order they come. A block with no timing line, or
/// with nothing said, is not a cue and is left out.
pub fn parse(subrip: &str) -> Vec<Cue> {
    subrip
        .replace("\r\n", "\n")
        .split("\n\n")
        .filter_map(|block| {
            let mut lines = block.lines().map(str::trim).filter(|line| !line.is_empty());
            let timing = lines.by_ref().find(|line| line.contains(" --> "))?;
            let text = lines.collect::<Vec<_>>().join(" ");
            (!text.is_empty()).then(|| Cue { timing: timing.to_string(), text })
        })
        .collect()
}

/// The file those cues make, numbered from one.
pub fn render(cues: &[Cue]) -> String {
    cues.iter()
        .enumerate()
        .map(|(position, cue)| format!("{}\n{}\n{}\n\n", position + 1, cue.timing, cue.text))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "1\n00:00:00,000 --> 00:00:11,000\n And so, my fellow Americans.\n\n2\n00:00:11,000 --> 00:00:14,000\n Hello\n there.\n\n";

    #[test]
    fn a_file_is_read_cue_by_cue_and_its_lines_joined() {
        let cues = parse(FILE);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].timing, "00:00:00,000 --> 00:00:11,000");
        assert_eq!(cues[0].text, "And so, my fellow Americans.");
        assert_eq!(cues[1].text, "Hello there.");
    }

    #[test]
    fn a_file_written_by_a_windows_editor_reads_the_same() {
        assert_eq!(parse(&FILE.replace('\n', "\r\n")), parse(FILE));
    }

    #[test]
    fn what_is_written_is_read_back_and_numbered_from_one() {
        let cues = parse(FILE);
        let written = render(&cues);
        assert!(written.starts_with("1\n00:00:00,000 --> 00:00:11,000\nAnd so"));
        assert!(written.contains("\n2\n00:00:11,000"));
        assert_eq!(parse(&written), cues);
    }

    #[test]
    fn a_block_that_says_nothing_is_not_a_cue() {
        assert!(parse("1\n00:00:00,000 --> 00:00:01,000\n\n\nnot a cue at all\n\n").is_empty());
        assert!(parse("").is_empty());
    }
}
