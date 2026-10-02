//! Cutting a line of speech into sentences.

/// Words that end in a full stop without ending the sentence.
const ABBREVIATIONS: [&str; 9] = ["mr", "mrs", "ms", "dr", "st", "jr", "sr", "vs", "etc"];

/// The sentences of a line, in order.
///
/// A sentence ends at a full stop, a question mark or an exclamation mark that
/// is followed by a space and then by a capital letter, a digit or a quote.
/// Not after an abbreviation, nor after a single letter (an initial): a wrong cut costs a worse translation, a missed one costs
/// nothing, so the doubtful cases are left whole.
pub fn sentences(text: &str) -> Vec<String> {
    let characters: Vec<char> = text.trim().chars().collect();
    let mut found = Vec::new();
    let mut from = 0;
    for position in 0..characters.len() {
        if !matches!(characters[position], '.' | '!' | '?' | '…') {
            continue;
        }
        let mut next = position + 1;
        while next < characters.len() && matches!(characters[next], '.' | '!' | '?' | '"' | '\'' | '”' | '’') {
            next += 1;
        }
        let spaced = characters.get(next).is_some_and(|c| c.is_whitespace());
        let starts_one = characters[next..]
            .iter()
            .find(|c| !c.is_whitespace())
            .is_some_and(|c| c.is_uppercase() || c.is_ascii_digit() || matches!(c, '"' | '“' | '¿' | '¡'));
        if spaced && starts_one && !abbreviation_before(&characters[from..position], characters[position]) {
            found.push(characters[from..next].iter().collect::<String>().trim().to_string());
            from = next;
        }
    }
    let rest: String = characters[from..].iter().collect::<String>().trim().to_string();
    if !rest.is_empty() {
        found.push(rest);
    }
    found
}

/// Whether the full stop that ends `before` belongs to an abbreviation or an
/// initial rather than closing a sentence.
fn abbreviation_before(before: &[char], mark: char) -> bool {
    if mark != '.' {
        return false;
    }
    let word: String = before
        .iter()
        .rev()
        .take_while(|c| c.is_alphanumeric() || **c == '.')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    word.chars().filter(|c| c.is_alphabetic()).count() == 1
        || ABBREVIATIONS.contains(&word.to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_cut_where_a_sentence_ends() {
        assert_eq!(
            sentences("Wait, hold on a second. Who left the door open? Nobody!"),
            ["Wait, hold on a second.", "Who left the door open?", "Nobody!"]
        );
    }

    #[test]
    fn one_sentence_stays_whole_even_with_commas_and_a_final_mark() {
        assert_eq!(sentences("Well, you know, it is what it is."), ["Well, you know, it is what it is."]);
        assert_eq!(sentences("  Hello there.  "), ["Hello there."]);
    }

    #[test]
    fn abbreviations_and_initials_do_not_end_a_sentence() {
        assert_eq!(sentences("Dr. Smith is here. Mr. Jones is not."), ["Dr. Smith is here.", "Mr. Jones is not."]);
        assert_eq!(sentences("J. K. Smith arrived."), ["J. K. Smith arrived."]);
        assert_eq!(sentences("The U.S. economy is strong."), ["The U.S. economy is strong."]);
    }

    #[test]
    fn a_lower_case_word_after_the_stop_means_the_sentence_goes_on() {
        assert_eq!(sentences("Hmm... maybe not."), ["Hmm... maybe not."]);
        assert_eq!(sentences("It cost 3.50 dollars."), ["It cost 3.50 dollars."]);
    }

    #[test]
    fn a_closing_quote_stays_with_its_sentence() {
        assert_eq!(sentences("He said \"stop.\" Then he left."), ["He said \"stop.\"", "Then he left."]);
    }

    #[test]
    fn nothing_said_gives_no_sentence() {
        assert!(sentences("   ").is_empty());
        assert!(sentences("").is_empty());
    }
}
