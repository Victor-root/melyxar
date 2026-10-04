//! Sending a notification: who receives it, what each of them keeps of it,
//! and telling their pages.

use std::collections::HashMap;

use melyxar_core::id::{LibraryId, NotificationId, UserId, WorkId};
use melyxar_core::time::Timestamp;
use melyxar_core::user::User;
use melyxar_database::notifications::{Channels, NewNotification, Notification};
use serde::{Deserialize, Serialize};

use super::live::{tell, Change};
use super::{choices, with_posters};
use crate::{AppState, Result};

/// How long before its time a maintenance is recalled.
pub(super) const RECALLED_BEFORE: time::Duration = time::Duration::hours(1);

/// What a notification is about, which is what each account chooses by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A message from the administrator, or a trial of one.
    Message,
    /// A message from the administrator about a time the server will be
    /// away. Always mandatory.
    Maintenance,
    /// Films or episodes that arrived in a library.
    NewContent,
    /// What a deletion did.
    Deletion,
    /// A title asked for, and what became of it.
    Request,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Maintenance => "maintenance",
            Self::NewContent => "new_content",
            Self::Deletion => "deletion",
            Self::Request => "request",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "message" => Some(Self::Message),
            "maintenance" => Some(Self::Maintenance),
            "new_content" => Some(Self::NewContent),
            "deletion" => Some(Self::Deletion),
            "request" => Some(Self::Request),
            _ => None,
        }
    }
}

/// The colour a notification wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Ok,
    Attention,
    Trouble,
    News,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Attention => "attention",
            Self::Trouble => "trouble",
            Self::News => "news",
        }
    }
}

/// A series some episodes arrived in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeriesArrived {
    pub id: WorkId,
    pub title: String,
    pub episodes: usize,
}

/// What happened to a title asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "what", rename_all = "snake_case")]
pub enum RequestNews {
    /// An account asked for it, told to the administrators.
    Asked {
        by: String,
        /// The seasons asked for; none for a film or a whole series.
        seasons: Vec<i32>,
    },
    Accepted,
    Refused {
        answer: String,
    },
    /// It is in a library now.
    Added,
}

/// What a notification says, kept as it is and translated when shown. A
/// message is written by the administrator in one language for everybody.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Said {
    Message {
        title: String,
        text: String,
    },
    Maintenance {
        title: String,
        text: String,
    },
    NewContent {
        library: LibraryId,
        library_name: String,
        films: usize,
        /// The first few, to be named.
        film_titles: Vec<String>,
        series: Vec<SeriesArrived>,
    },
    Deletion {
        /// The first few, to be named.
        titles: Vec<String>,
        works: i64,
        /// How many files were found gone from the disk, when it was asked
        /// to lose them.
        checked_gone: Option<usize>,
    },
    Request {
        title: String,
        news: RequestNews,
    },
}

impl Said {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Message { .. } => Kind::Message,
            Self::Maintenance { .. } => Kind::Maintenance,
            Self::NewContent { .. } => Kind::NewContent,
            Self::Deletion { .. } => Kind::Deletion,
            Self::Request { .. } => Kind::Request,
        }
    }
}

/// Who a notification is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Audience {
    Administrators,
    Everyone,
    Accounts(Vec<UserId>),
}

/// One notification on its way.
#[derive(Debug, Clone, PartialEq)]
pub struct Outgoing {
    pub said: Said,
    pub level: Level,
    pub audience: Audience,
    /// The work it shows, whose poster it wears.
    pub work: Option<WorkId>,
    pub priority: bool,
    pub mandatory: bool,
    pub sticky: bool,
    /// How long it stays on the screen; absent, its level decides.
    pub shown_for: Option<time::Duration>,
    /// When the maintenance it announces happens.
    pub due_at: Option<Timestamp>,
}

impl Outgoing {
    /// The usual: on the screen for as long as its level says, for no one
    /// in particular yet.
    pub fn new(said: Said, level: Level, audience: Audience) -> Self {
        Self {
            said,
            level,
            audience,
            work: None,
            priority: false,
            mandatory: false,
            sticky: false,
            shown_for: None,
            due_at: None,
        }
    }

    fn is_mandatory(&self) -> bool {
        self.mandatory || self.said.kind() == Kind::Maintenance
    }
}

/// The accounts that may read a library and did not choose to stop hearing
/// about it.
pub(super) async fn readers_of(state: &AppState, library: LibraryId) -> Result<Vec<User>> {
    let database = state.database();
    let deaf = database.deaf_to_library(library).await?;
    Ok(database
        .list_users()
        .await?
        .into_iter()
        .filter(|user| user.permissions.may_access_library(library) && !deaf.contains(&user.id))
        .collect())
}

/// The accounts an audience stands for.
async fn recipients(state: &AppState, audience: &Audience) -> Result<Vec<UserId>> {
    let keep = |user: &User| match audience {
        Audience::Administrators => user.permissions.is_administrator,
        Audience::Everyone => true,
        Audience::Accounts(ids) => ids.contains(&user.id),
    };
    Ok(state
        .database()
        .list_users()
        .await?
        .iter()
        .filter(|user| keep(user))
        .map(|user| user.id)
        .collect())
}

/// A notification said on the screen only, never written down: the account
/// switched its bell off for this kind.
fn passing(user: UserId, new: &NewNotification<'_>) -> Notification {
    Notification {
        id: NotificationId::new(),
        user_id: user,
        kind: new.kind.to_string(),
        level: new.level.to_string(),
        data: new.data.to_string(),
        work_id: new.work_id,
        priority: new.priority,
        mandatory: new.mandatory,
        sticky: new.sticky,
        shown_for_ms: new.shown_for_ms,
        due_at: new.due_at,
        created_at: new.created_at,
        read_at: None,
    }
}

/// Sends a notification: kept by every account that keeps this kind, shown
/// on the screen of every one that wants it there, and told to all their
/// pages at once. Answers what was kept.
pub async fn send(state: &AppState, outgoing: Outgoing) -> Result<Vec<Notification>> {
    let kind = outgoing.said.kind();
    let mandatory = outgoing.is_mandatory();
    let recipients = recipients(state, &outgoing.audience).await?;
    if recipients.is_empty() {
        return Ok(Vec::new());
    }
    let channels: HashMap<UserId, Channels> = match mandatory {
        true => recipients
            .iter()
            .map(|user| (*user, Channels { bell: true, screen: true }))
            .collect(),
        false => choices::channels_for(state, kind, &recipients).await?,
    };

    let now = melyxar_core::time::now();
    let data = serde_json::to_string(&outgoing.said)
        .map_err(|error| melyxar_core::Error::internal(error.to_string()))?;
    let new = NewNotification {
        kind: kind.as_str(),
        level: outgoing.level.as_str(),
        data: &data,
        work_id: outgoing.work,
        priority: outgoing.priority,
        mandatory,
        sticky: outgoing.sticky,
        shown_for_ms: outgoing.shown_for.map(|shown| shown.whole_milliseconds() as i64),
        due_at: outgoing.due_at,
        reminded: outgoing.due_at.is_some_and(|due| due - now <= RECALLED_BEFORE),
        created_at: now,
    };

    let keeping: Vec<UserId> = recipients
        .iter()
        .copied()
        .filter(|user| channels[user].bell)
        .collect();
    let kept = state.database().add_notifications(&keeping, &new).await?;
    let only_shown = recipients
        .iter()
        .filter(|user| !channels[user].bell && channels[user].screen)
        .map(|&user| passing(user, &new));
    let said: Vec<Notification> = kept.iter().cloned().chain(only_shown).collect();

    for told in with_posters(state, said).await? {
        let user = told.notification.user_id;
        let change = Change::Arrived {
            kept: channels[&user].bell,
            screen: channels[&user].screen,
            told,
        };
        tell(state, user, change);
    }
    Ok(kept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::live::follow;
    use crate::an_empty_server;
    use melyxar_core::user::Permissions;

    fn hello() -> Said {
        Said::Message {
            title: "Hello".to_string(),
            text: "A word.".to_string(),
        }
    }

    #[test]
    fn what_is_said_is_kept_with_its_kind() {
        assert_eq!(
            serde_json::to_value(hello()).expect("written"),
            serde_json::json!({ "kind": "message", "title": "Hello", "text": "A word." })
        );
        for kind in [Kind::Message, Kind::Maintenance, Kind::NewContent, Kind::Deletion, Kind::Request] {
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
        }
    }

    #[tokio::test]
    async fn each_account_keeps_and_sees_what_it_chose_and_a_mandatory_one_reaches_all() {
        let (_held, state) = an_empty_server().await;
        let database = state.database();
        let admin = database
            .create_user("admin", None, &Permissions::administrator())
            .await
            .expect("account")
            .id;
        let viewer = database
            .create_user("viewer", None, &Permissions::viewer())
            .await
            .expect("account")
            .id;
        let silent = database
            .create_user("silent", None, &Permissions::viewer())
            .await
            .expect("account")
            .id;
        choices::choose(&state, viewer, Kind::Message, Channels { bell: false, screen: true })
            .await
            .expect("chosen");
        choices::choose(&state, silent, Kind::Message, Channels { bell: false, screen: false })
            .await
            .expect("chosen");

        let mut line = follow(&state);
        let kept = send(&state, Outgoing::new(hello(), Level::Ok, Audience::Everyone))
            .await
            .expect("sent");
        assert_eq!(kept.len(), 1, "only the administrator keeps it");
        assert_eq!(kept[0].user_id, admin);

        let mut arrived = Vec::new();
        while let Ok(news) = line.try_recv() {
            if let Change::Arrived { kept, screen, .. } = news.change {
                arrived.push((news.user, kept, screen));
            }
        }
        arrived.sort();
        let mut expected = vec![(admin, true, true), (viewer, false, true)];
        expected.sort();
        assert_eq!(arrived, expected, "the account that wants nothing is told nothing");

        let maintenance = Said::Maintenance {
            title: "Down tonight".to_string(),
            text: String::new(),
        };
        let due = melyxar_core::time::now() + time::Duration::minutes(30);
        let kept = send(
            &state,
            Outgoing {
                due_at: Some(due),
                ..Outgoing::new(maintenance, Level::Attention, Audience::Everyone)
            },
        )
        .await
        .expect("sent");
        assert_eq!(kept.len(), 3, "nobody may switch a maintenance off");
        assert!(kept.iter().all(|one| one.mandatory));
    }

    #[tokio::test]
    async fn a_library_is_heard_about_by_who_may_read_it_and_did_not_say_otherwise() {
        let (_held, state) = an_empty_server().await;
        let database = state.database();
        let films = database
            .create_library("Films", melyxar_core::library::LibraryKind::Movies, "fr", &[])
            .await
            .expect("library")
            .id;
        let sees = database
            .create_user("sees", None, &Permissions::viewer())
            .await
            .expect("account");
        let deaf = database
            .create_user("deaf", None, &Permissions::viewer())
            .await
            .expect("account");
        let shut_out = Permissions {
            sees_every_library: false,
            allowed_libraries: Vec::new(),
            ..Permissions::viewer()
        };
        database
            .create_user("shut out", None, &shut_out)
            .await
            .expect("account");
        database
            .set_library_announcement_choice(deaf.id, films, false)
            .await
            .expect("chosen");

        let readers = readers_of(&state, films).await.expect("found");
        assert_eq!(readers.iter().map(|user| user.id).collect::<Vec<_>>(), vec![sees.id]);
    }
}
