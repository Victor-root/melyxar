//! Passwords, and the tokens that stand in for them once somebody is in.
//!
//! Everything here is pure: no database, no HTTP, no clock. It is the part of
//! signing in that has to be *right* rather than the part that has to be wired
//! up, so it lives where it can be tested on its own and read in one sitting.
//!
//! Two different jobs, deliberately not done the same way.
//!
//! A password is turned into a stored form by a function built to be slow and
//! to eat memory, so that a stolen database is worth as little as possible:
//! whoever holds it has to spend that cost again for every guess. It is run
//! once, when somebody signs in, and a fraction of a second there is nobody's
//! problem.
//!
//! A session token is not guessed, it is drawn: thirty two bytes of randomness
//! from the machine, which nothing walks its way through. So what is stored
//! beside it is a plain fingerprint, and it is checked on every single request,
//! every picture and every segment of a film. Hashing a token the way a
//! password is hashed would put a tenth of a second between a viewer and each
//! segment of what they are watching.

#![forbid(unsafe_code)]

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("a password of fewer than {SHORTEST_PASSWORD} characters")]
    PasswordTooShort,
    #[error("the password could not be hashed: {0}")]
    CouldNotHash(String),
    #[error("the machine would not give any randomness: {0}")]
    NoRandomness(String),
}

pub type Result<T> = std::result::Result<T, AuthError>;

/// The fewest characters a password may hold.
///
/// Length, and nothing else asked of it. Rules about capitals and digits push
/// people towards one predictable word with a number stuck on the end, which
/// is weaker than the long ordinary phrase they would have picked on their
/// own.
pub const SHORTEST_PASSWORD: usize = 8;

/// How many bytes of randomness a session token carries.
///
/// Thirty two, which is more than anything could work through before the sun
/// goes out. It is written down here because the whole reason the fingerprint
/// below is a plain one is that there is nothing in a token to guess.
const TOKEN_BYTES: usize = 32;

/// Turns a password into the form that is stored, and never the other way.
///
/// The stored form carries its own salt and its own settings inside it, which
/// is what lets a password checked today still be checked after the settings
/// are made heavier tomorrow.
pub fn hash_password(password: &str) -> Result<String> {
    // Counted in characters rather than bytes: a phrase of eight accented
    // letters is eight characters and sixteen bytes, and refusing it while
    // accepting eight plain ones would be refusing people their own language.
    if password.chars().count() < SHORTEST_PASSWORD {
        return Err(AuthError::PasswordTooShort);
    }

    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hashed| hashed.to_string())
        .map_err(|error| AuthError::CouldNotHash(error.to_string()))
}

/// Whether a password is the one behind a stored form.
///
/// Answers no rather than failing when the stored form cannot be read at all:
/// a row written by hand, or by a version that stored something else, is a
/// password nobody can present, and there is nothing else to say about it.
pub fn password_matches(password: &str, stored: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// What a signed in client holds, and hands back on every request.
///
/// Never written to a log or a journal: it is the password for as long as it
/// lives, which is why it will not print itself.
#[derive(Clone)]
pub struct SessionToken(String);

impl SessionToken {
    /// Draws a new one from the machine's own randomness.
    pub fn new() -> Result<Self> {
        let mut drawn = [0u8; TOKEN_BYTES];
        getrandom::fill(&mut drawn).map_err(|error| AuthError::NoRandomness(error.to_string()))?;
        Ok(Self(as_hexadecimal(&drawn)))
    }

    /// What goes to the client, and only to the client.
    pub fn as_text(&self) -> &str {
        &self.0
    }

    /// What is written down beside the session instead of the token itself.
    pub fn fingerprint(&self) -> String {
        fingerprint_of(&self.0)
    }
}

/// The fingerprint of a token somebody presented, for looking its session up.
///
/// The server keeps fingerprints and never tokens, so a copy of the database
/// hands over nobody's session. Nothing here compares two of them: the
/// fingerprint is what the stored session is *found* by, so the comparison is
/// an index walk rather than a line of ours that could give its answer away by
/// how long it took.
pub fn fingerprint_of(token: &str) -> String {
    as_hexadecimal(&Sha256::digest(token.as_bytes()))
}

impl std::fmt::Debug for SessionToken {
    /// Says that there is one without saying which.
    ///
    /// Written out because the derived one would print the token, and one
    /// error line carrying a request somebody's session travelled in is a
    /// session anybody reading that line can walk into.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SessionToken(not shown)")
    }
}

/// Bytes as the text they are written down in.
fn as_hexadecimal(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut written = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Two characters per byte, always: a byte below sixteen written as one
        // character would let two different runs of bytes be written the same.
        // Writing into a string cannot fail, so there is no failure to carry.
        let _ = write!(written, "{byte:02x}");
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    const A_PASSWORD: &str = "quiet harbour";

    #[test]
    fn a_password_is_never_stored_as_it_was_typed() {
        let stored = hash_password(A_PASSWORD).expect("hashed");
        assert!(!stored.contains(A_PASSWORD));
        assert!(
            stored.starts_with("$argon2"),
            "the stored form says which function made it: {stored}"
        );
    }

    #[test]
    fn the_same_password_is_stored_differently_every_time() {
        // Each one carries its own salt, so two people who chose the same
        // password cannot be spotted as having done so, and one broken
        // password does not break the other.
        let once = hash_password(A_PASSWORD).expect("hashed");
        let twice = hash_password(A_PASSWORD).expect("hashed");
        assert_ne!(once, twice);
        assert!(password_matches(A_PASSWORD, &once));
        assert!(password_matches(A_PASSWORD, &twice));
    }

    #[test]
    fn a_password_matches_its_own_stored_form_and_nothing_else() {
        let stored = hash_password(A_PASSWORD).expect("hashed");
        assert!(password_matches(A_PASSWORD, &stored));
        assert!(!password_matches("quiet harbours", &stored));
        assert!(!password_matches("Quiet harbour", &stored));
        assert!(!password_matches("", &stored));
    }

    #[test]
    fn a_stored_form_nobody_can_read_matches_nothing_rather_than_failing() {
        // A row written by hand, or by a version that stored something else.
        // It must lock that one account rather than take the server down.
        for nonsense in ["", "not a hash at all", "$argon2id$v=19$nonsense"] {
            assert!(!password_matches(A_PASSWORD, nonsense));
        }
    }

    #[test]
    fn a_password_shorter_than_the_rule_is_refused() {
        assert!(matches!(
            hash_password("short"),
            Err(AuthError::PasswordTooShort)
        ));
        assert!(hash_password(&"a".repeat(SHORTEST_PASSWORD)).is_ok());
    }

    #[test]
    fn the_length_of_a_password_is_counted_in_characters() {
        // Eight accented letters are eight characters and sixteen bytes.
        // Counted in bytes this would pass for the wrong reason, and seven of
        // them would pass too, which is the fault worth catching.
        let eight = "éééééééé";
        assert_eq!(eight.chars().count(), SHORTEST_PASSWORD);
        assert!(hash_password(eight).is_ok());

        let seven = "ééééééé";
        assert!(seven.len() > SHORTEST_PASSWORD, "longer in bytes");
        assert!(matches!(
            hash_password(seven),
            Err(AuthError::PasswordTooShort)
        ));
    }

    #[test]
    fn two_tokens_are_never_the_same() {
        let once = SessionToken::new().expect("drawn");
        let twice = SessionToken::new().expect("drawn");
        assert_ne!(once.as_text(), twice.as_text());
        assert_eq!(once.as_text().len(), TOKEN_BYTES * 2);
    }

    #[test]
    fn a_token_is_found_by_its_fingerprint_and_never_stored_itself() {
        let token = SessionToken::new().expect("drawn");
        assert_ne!(token.fingerprint(), token.as_text());
        assert_eq!(token.fingerprint(), fingerprint_of(token.as_text()));

        let another = SessionToken::new().expect("drawn");
        assert_ne!(token.fingerprint(), another.fingerprint());
    }

    #[test]
    fn a_token_does_not_print_itself() {
        // One error line carrying the request a session travelled in is a
        // session anybody reading that line can walk into.
        let token = SessionToken::new().expect("drawn");
        let printed = format!("{token:?}");
        assert!(!printed.contains(token.as_text()), "printed as {printed}");
    }

    #[test]
    fn bytes_are_written_two_characters_at_a_time() {
        assert_eq!(as_hexadecimal(&[0, 15, 16, 255]), "000f10ff");
    }
}
