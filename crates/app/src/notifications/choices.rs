//! What each account chose to be told, and how: kind by kind, the bell and
//! the screen; its quiet hours; the libraries it hears about. An account that
//! chose nothing for a kind follows what the administrator set for every
//! account.

use std::collections::HashMap;

use melyxar_core::id::{LibraryId, UserId};
use melyxar_core::user::User;
pub use melyxar_database::notifications::{Channels, NotificationSettings};

use super::live::{tell, Change};
use super::Kind;
use crate::{AppState, Result};

/// The kinds an account may choose about, in the order they are listed. A
/// maintenance is not one of them: nobody may switch it off.
pub const CHOOSABLE: [Kind; 3] = [Kind::NewContent, Kind::Message, Kind::Deletion];

/// Minutes in a day, the clock the quiet hours are read on.
const A_DAY_IN_MINUTES: i32 = 24 * 60;

/// What one account gets of one kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindChoice {
    pub kind: Kind,
    pub channels: Channels,
    /// Whether the account chose, rather than following the default.
    pub chosen: bool,
}

/// One library an account may hear about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryChoice {
    pub id: LibraryId,
    pub name: String,
    pub announced: bool,
}

/// Everything an account chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountChoices {
    pub kinds: Vec<KindChoice>,
    pub settings: NotificationSettings,
    /// The libraries that announce what arrives and that it may read.
    pub libraries: Vec<LibraryChoice>,
}

/// What a kind gets when nothing was chosen: the default, or both channels
/// for a kind the administrator never set.
fn by_default(defaults: &HashMap<String, Channels>, kind: Kind) -> Channels {
    defaults.get(kind.as_str()).copied().unwrap_or(Channels {
        bell: true,
        screen: true,
    })
}

/// Quiet hours are both ends or neither, each a minute of the day. Starting
/// and ending at the same minute would be a quiet that never begins.
fn quiet_hours_hold(settings: &NotificationSettings) -> bool {
    let minute = |value: i32| (0..A_DAY_IN_MINUTES).contains(&value);
    match (settings.quiet_from, settings.quiet_until) {
        (None, None) => true,
        (Some(from), Some(until)) => minute(from) && minute(until) && from != until,
        _ => false,
    }
}

pub async fn of(state: &AppState, who: &User) -> Result<AccountChoices> {
    let database = state.database();
    let defaults = database.notification_defaults().await?;
    let chosen = database.notification_choices(who.id).await?;
    let kinds = CHOOSABLE
        .iter()
        .map(|&kind| match chosen.get(kind.as_str()) {
            Some(&channels) => KindChoice {
                kind,
                channels,
                chosen: true,
            },
            None => KindChoice {
                kind,
                channels: by_default(&defaults, kind),
                chosen: false,
            },
        })
        .collect();

    let heard = database.library_announcement_choices(who.id).await?;
    let libraries = super::news::announcing(state)
        .await?
        .into_iter()
        .filter(|library| who.permissions.may_access_library(library.id))
        .map(|library| LibraryChoice {
            announced: heard.get(&library.id).copied().unwrap_or(true),
            id: library.id,
            name: library.name,
        })
        .collect();

    Ok(AccountChoices {
        kinds,
        settings: database.notification_settings(who.id).await?,
        libraries,
    })
}

fn not_choosable(kind: Kind) -> crate::AppError {
    melyxar_core::Error::invalid_input(format!("nobody chooses about {}", kind.as_str())).into()
}

/// What an account gets of one kind from now on.
pub async fn choose(state: &AppState, who: UserId, kind: Kind, channels: Channels) -> Result<()> {
    if !CHOOSABLE.contains(&kind) {
        return Err(not_choosable(kind));
    }
    state.database().set_notification_choice(who, kind.as_str(), channels).await?;
    tell(state, who, Change::ChoicesChanged);
    Ok(())
}

/// An account's quiet hours and what still shows through them.
pub async fn settle(state: &AppState, who: UserId, settings: &NotificationSettings) -> Result<()> {
    if !quiet_hours_hold(settings) {
        return Err(melyxar_core::Error::invalid_input(
            "quiet hours are two different minutes of the day, or none",
        )
        .into());
    }
    state.database().set_notification_settings(who, settings).await?;
    tell(state, who, Change::ChoicesChanged);
    Ok(())
}

/// Whether an account hears about what arrives in a library it may read.
pub async fn hear_about(state: &AppState, who: &User, library: LibraryId, announced: bool) -> Result<()> {
    crate::reach::may_read(who, library)?;
    state
        .database()
        .set_library_announcement_choice(who.id, library, announced)
        .await?;
    tell(state, who.id, Change::ChoicesChanged);
    Ok(())
}

/// What every account that chose nothing gets, kind by kind.
pub async fn defaults(state: &AppState) -> Result<Vec<(Kind, Channels)>> {
    let defaults = state.database().notification_defaults().await?;
    Ok(CHOOSABLE
        .iter()
        .map(|&kind| (kind, by_default(&defaults, kind)))
        .collect())
}

pub async fn set_default(state: &AppState, kind: Kind, channels: Channels) -> Result<()> {
    if !CHOOSABLE.contains(&kind) {
        return Err(not_choosable(kind));
    }
    state.database().set_notification_default(kind.as_str(), channels).await?;
    Ok(())
}

/// What each of these accounts gets of one kind.
pub(super) async fn channels_for(
    state: &AppState,
    kind: Kind,
    recipients: &[UserId],
) -> Result<HashMap<UserId, Channels>> {
    let database = state.database();
    let fallback = by_default(&database.notification_defaults().await?, kind);
    let chosen = database.notification_choices_for(kind.as_str()).await?;
    Ok(recipients
        .iter()
        .map(|user| (*user, chosen.get(user).copied().unwrap_or(fallback)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_never_set_reaches_both_channels() {
        let mut defaults = HashMap::new();
        assert_eq!(
            by_default(&defaults, Kind::Message),
            Channels { bell: true, screen: true }
        );
        let quiet = Channels { bell: true, screen: false };
        defaults.insert("message".to_string(), quiet);
        assert_eq!(by_default(&defaults, Kind::Message), quiet);
    }

    #[test]
    fn quiet_hours_are_two_different_minutes_of_the_day_or_none() {
        let with = |from, until| NotificationSettings {
            quiet_from: from,
            quiet_until: until,
            ..NotificationSettings::default()
        };
        assert!(quiet_hours_hold(&with(None, None)));
        assert!(quiet_hours_hold(&with(Some(22 * 60), Some(7 * 60))), "across midnight");
        assert!(quiet_hours_hold(&with(Some(0), Some(1439))));
        assert!(!quiet_hours_hold(&with(Some(60), None)), "one end only");
        assert!(!quiet_hours_hold(&with(Some(60), Some(60))), "never begins");
        assert!(!quiet_hours_hold(&with(Some(-1), Some(60))));
        assert!(!quiet_hours_hold(&with(Some(60), Some(1440))));
    }

    #[test]
    fn a_maintenance_cannot_be_chosen_about() {
        assert!(!CHOOSABLE.contains(&Kind::Maintenance));
    }
}
