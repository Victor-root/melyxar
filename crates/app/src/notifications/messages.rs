//! What the administrator writes to the accounts: a message, a trial of
//! one, or a maintenance announced for a given time.

use melyxar_core::time::Timestamp;

use super::{send, Audience, Level, Outgoing, Said};
use crate::{AppState, Result};

/// The longest title, in characters: it is one line in the corner.
const TITLE_AT_MOST: usize = 120;
/// The longest text, in characters.
const TEXT_AT_MOST: usize = 2_000;
/// How long a message may stay on the screen, in seconds.
const SHOWN_FOR_SECONDS: std::ops::RangeInclusive<u32> = 2..=120;

/// A message as the administrator wrote it.
#[derive(Debug, Clone, PartialEq)]
pub struct Written {
    pub level: Level,
    pub title: String,
    pub text: String,
    pub audience: Audience,
    /// Absent, its level decides.
    pub shown_for_seconds: Option<u32>,
    pub sticky: bool,
    pub priority: bool,
    pub mandatory: bool,
    /// When the maintenance it announces happens; absent for a message.
    pub due_at: Option<Timestamp>,
}

fn refused(why: &str) -> crate::AppError {
    melyxar_core::Error::invalid_input(why).into()
}

/// What is wrong with a message, if anything.
fn check(written: &Written, now: Timestamp) -> Result<()> {
    let title = written.title.trim();
    if title.is_empty() {
        return Err(refused("a message needs a title"));
    }
    if title.chars().count() > TITLE_AT_MOST || written.text.chars().count() > TEXT_AT_MOST {
        return Err(refused("a message this long would not fit on the screen"));
    }
    if written
        .shown_for_seconds
        .is_some_and(|seconds| !SHOWN_FOR_SECONDS.contains(&seconds))
    {
        return Err(refused("a message stays between two seconds and two minutes"));
    }
    if written.due_at.is_some_and(|due| due <= now) {
        return Err(refused("a maintenance is announced for a time still to come"));
    }
    if matches!(&written.audience, Audience::Accounts(ids) if ids.is_empty()) {
        return Err(refused("a message is sent to somebody"));
    }
    Ok(())
}

/// Sends what the administrator wrote.
pub async fn write(state: &AppState, written: Written) -> Result<()> {
    check(&written, melyxar_core::time::now())?;
    let title = written.title.trim().to_string();
    let text = written.text.trim().to_string();
    let said = match written.due_at {
        Some(_) => Said::Maintenance { title, text },
        None => Said::Message { title, text },
    };
    let outgoing = Outgoing {
        priority: written.priority,
        mandatory: written.mandatory,
        sticky: written.sticky,
        shown_for: written
            .shown_for_seconds
            .map(|seconds| time::Duration::seconds(i64::from(seconds))),
        due_at: written.due_at,
        ..Outgoing::new(said, written.level, written.audience)
    };
    send(state, outgoing).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    const NOON: Timestamp = datetime!(2026-10-03 12:00 UTC);

    fn a_message() -> Written {
        Written {
            level: Level::Ok,
            title: "Hello".to_string(),
            text: String::new(),
            audience: Audience::Administrators,
            shown_for_seconds: None,
            sticky: false,
            priority: false,
            mandatory: false,
            due_at: None,
        }
    }

    #[test]
    fn a_message_is_refused_for_what_would_not_show_or_reach_anybody() {
        assert!(check(&a_message(), NOON).is_ok());
        let refused = |written: Written| check(&written, NOON).is_err();
        assert!(refused(Written { title: "   ".to_string(), ..a_message() }));
        assert!(refused(Written { title: "a".repeat(TITLE_AT_MOST + 1), ..a_message() }));
        assert!(refused(Written { text: "a".repeat(TEXT_AT_MOST + 1), ..a_message() }));
        assert!(refused(Written { shown_for_seconds: Some(1), ..a_message() }));
        assert!(refused(Written { shown_for_seconds: Some(121), ..a_message() }));
        assert!(refused(Written { due_at: Some(NOON), ..a_message() }), "already now");
        assert!(refused(Written { audience: Audience::Accounts(Vec::new()), ..a_message() }));
        assert!(check(&Written { due_at: Some(NOON + time::Duration::hours(2)), ..a_message() }, NOON).is_ok());
    }
}
