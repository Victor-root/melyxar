//! What the server is called, as its administrator names it.

use crate::{AppError, AppState};

/// The longest a server's name may be, the same as a library's: it is carried
/// in a browser's tab and under an installed icon, where a longer one is cut.
pub const LONGEST_NAME: usize = 60;

/// Why a name was not taken, in a word the interface turns into a sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NameNeeded,
    NameTooLong,
}

impl Refused {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NameNeeded => "name_needed",
            Self::NameTooLong => "name_too_long",
        }
    }

    /// Every one of them, so a test can check each has words on the screen.
    pub const ALL: [Self; 2] = [Self::NameNeeded, Self::NameTooLong];
}

/// What can go wrong: a refusal to put into words, or the server itself.
#[derive(Debug, thiserror::Error)]
pub enum Trouble {
    #[error("refused: {}", .0.as_str())]
    Refused(Refused),
    #[error(transparent)]
    Failed(#[from] AppError),
}

impl From<melyxar_database::DatabaseError> for Trouble {
    fn from(error: melyxar_database::DatabaseError) -> Self {
        Self::Failed(AppError::from(error))
    }
}

/// What the server is called.
pub async fn name(state: &AppState) -> Result<String, AppError> {
    Ok(state.database().server_settings().await?.server_name)
}

/// Calls the server something else, and answers the name as it was kept.
pub async fn rename(state: &AppState, asked: &str) -> Result<String, Trouble> {
    let name = name_of(asked)?;
    state.database().set_server_name(&name).await?;
    Ok(name)
}

/// The name a server is to be called, or a refusal saying why not.
fn name_of(asked: &str) -> Result<String, Trouble> {
    let name = asked.trim();
    if name.is_empty() {
        return Err(Trouble::Refused(Refused::NameNeeded));
    }
    if name.chars().count() > LONGEST_NAME {
        return Err(Trouble::Refused(Refused::NameTooLong));
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refusal(asked: &str) -> Option<Refused> {
        match name_of(asked) {
            Err(Trouble::Refused(refused)) => Some(refused),
            _ => None,
        }
    }

    #[test]
    fn a_name_is_kept_without_the_spaces_around_it() {
        assert_eq!(name_of("  Home Cinema ").expect("taken"), "Home Cinema");
    }

    #[test]
    fn an_empty_name_is_refused() {
        assert_eq!(refusal("   "), Some(Refused::NameNeeded));
    }

    #[test]
    fn a_name_is_counted_in_letters_rather_than_bytes() {
        assert!(name_of(&"é".repeat(LONGEST_NAME)).is_ok());
        assert_eq!(
            refusal(&"é".repeat(LONGEST_NAME + 1)),
            Some(Refused::NameTooLong)
        );
    }
}
