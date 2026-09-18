//! Reading a piece of text the way a comparison needs it read.
//!
//! Below everything that reads text, because more than one crate does and
//! two foldings that disagree are two spellings of one word that stop
//! matching each other.

/// Replaces accented letters with their plain form.
///
/// Handles the letters that actually occur in the languages at hand rather
/// than pulling in a full normalisation library for a handful of characters.
///
/// An accent reaches us written one of two ways. Usually it is one character,
/// and the table below covers those. But a file can carry the plain letter
/// followed by the accent as a mark of its own, which looks identical on any
/// screen and is not the same text at all. Those marks are dropped, so that
/// the folded form is the same either way: without that, two spellings of one
/// title sort apart, compare as different, and one of them finds nothing at a
/// provider.
pub fn fold_accents(value: &str) -> String {
    value
        .chars()
        .filter(|c| !is_a_combining_mark(*c))
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'È' | 'É' | 'Ê' | 'Ë' => 'E',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            'ñ' => 'n',
            'Ñ' => 'N',
            'ÿ' => 'y',
            other => other,
        })
        .collect()
}

/// Whether a character is an accent written on its own, after the letter it
/// belongs to. The block below is the one Latin scripts use.
fn is_a_combining_mark(c: char) -> bool {
    ('\u{0300}'..='\u{036f}').contains(&c)
}
