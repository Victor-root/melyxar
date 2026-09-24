//! What deserves an administrator's look, gathered from everywhere. See the
//! decision on what to keep an eye on.
//!
//! Worked out every time it is asked for and never stored. A point born of
//! past events or of a count can be marked as seen, and is quiet until
//! something new happens. So can a disk nearly full, which is worth knowing
//! and rarely worth being told again: it stays quiet while it stays full,
//! and speaks again if it fills again after having had room. Every other
//! point describes something still wrong, cannot be seen away, and goes once
//! it is put right.

use std::collections::HashMap;

use melyxar_core::id::UserId;
use melyxar_core::time::Timestamp;
use serde::Serialize;

use crate::activity::{SIGN_IN_HELD_BACK, SIGN_IN_REFUSED, TASK_FAILED};
use crate::overview::Worry;
use crate::{AppState, Result};

/// How far back past events count.
const A_DAY: time::Duration = time::Duration::hours(24);

/// One thing to look at.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "point", rename_all = "snake_case")]
pub enum Point {
    /// A light of the summary that failed its check.
    Worry(Worry),
    /// Sign ins refused, or accounts held back, since the day began or since
    /// it was last seen.
    RefusedSignIns { count: i64 },
    /// Tasks that failed, over the same stretch.
    FailedTasks { count: i64 },
    /// Conversions producing their film more slowly than it plays, now.
    FallingBehind { count: usize },
    /// Works still waiting for a name.
    Unidentified { count: i64 },
}

/// How much a point asks of whoever reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Attention,
    Trouble,
}

/// A point as it is shown.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Shown {
    #[serde(flatten)]
    pub point: Point,
    pub state: State,
    /// Whether marking it as seen quiets it.
    pub may_be_seen: bool,
}

impl Point {
    /// What its being seen is written down under, for a point that can be.
    fn key(&self) -> Option<String> {
        match self {
            Self::RefusedSignIns { .. } => Some("refused_sign_ins".to_string()),
            Self::FailedTasks { .. } => Some("failed_tasks".to_string()),
            Self::Unidentified { .. } => Some("unidentified".to_string()),
            Self::Worry(Worry::DiskNearlyFull { mount, .. }) => Some(disk_key(mount)),
            Self::Worry(_) | Self::FallingBehind { .. } => None,
        }
    }

    fn state(&self) -> State {
        match self {
            Self::Worry(Worry::CardUnreachable | Worry::DiskNearlyFull { .. })
            | Self::RefusedSignIns { .. }
            | Self::FallingBehind { .. }
            | Self::Unidentified { .. } => State::Attention,
            Self::Worry(_) | Self::FailedTasks { .. } => State::Trouble,
        }
    }

    /// What it is marked at when seen: the instant for a count of events,
    /// the count itself for a count of works.
    fn mark_now(&self, now: Timestamp) -> Option<i64> {
        match self {
            Self::RefusedSignIns { .. } | Self::FailedTasks { .. } => Some(milliseconds(now)),
            Self::Unidentified { count } => Some(*count),
            // Seen or not, nothing to count: it is quiet while it lasts.
            Self::Worry(Worry::DiskNearlyFull { .. }) => Some(1),
            Self::Worry(_) | Self::FallingBehind { .. } => None,
        }
    }
}

/// What a disk nearly full is marked seen under, one per disk.
const DISK_FULL: &str = "disk_full:";

fn disk_key(mount: &str) -> String {
    format!("{DISK_FULL}{mount}")
}

fn milliseconds(at: Timestamp) -> i64 {
    i64::try_from(at.unix_timestamp_nanos() / 1_000_000).unwrap_or(i64::MAX)
}

fn from_milliseconds(mark: i64) -> Option<Timestamp> {
    Timestamp::from_unix_timestamp_nanos(i128::from(mark) * 1_000_000).ok()
}

/// Past events count over the last day, or since they were last seen when
/// that is later.
fn counted_since(seen: &HashMap<String, i64>, key: &str, now: Timestamp) -> Timestamp {
    let a_day_ago = now - A_DAY;
    seen.get(key)
        .and_then(|&mark| from_milliseconds(mark))
        .map_or(a_day_ago, |seen_at| seen_at.max(a_day_ago))
}

/// The lights of the summary that failed their check, less the disks nearly
/// full this administrator has already seen.
fn worries_unseen(worries: Vec<Worry>, seen: &HashMap<String, i64>) -> Vec<Point> {
    worries
        .into_iter()
        .filter(|worry| match worry {
            Worry::DiskNearlyFull { mount, .. } => !seen.contains_key(&disk_key(mount)),
            _ => true,
        })
        .map(Point::Worry)
        .collect()
}

/// The disks seen nearly full that have room again.
fn seen_with_room<'a>(worries: &[Worry], seen: &'a HashMap<String, i64>) -> Vec<&'a str> {
    seen.keys()
        .filter(|key| key.starts_with(DISK_FULL))
        .filter(|key| {
            !worries.iter().any(|worry| {
                matches!(worry, Worry::DiskNearlyFull { mount, .. } if disk_key(mount) == **key)
            })
        })
        .map(String::as_str)
        .collect()
}

/// Everything there is to look at, as it stands, in the order it matters.
async fn everything(
    state: &AppState,
    worries: Vec<Worry>,
    seen: &HashMap<String, i64>,
    now: Timestamp,
) -> Result<Vec<Point>> {
    let database = state.database();
    let mut points = worries_unseen(worries, seen);

    let failed = database
        .count_activity_since(&[TASK_FAILED], counted_since(seen, "failed_tasks", now))
        .await?;
    if failed > 0 {
        points.push(Point::FailedTasks { count: failed });
    }
    let behind = crate::watching::falling_behind(state).await;
    if behind > 0 {
        points.push(Point::FallingBehind { count: behind });
    }
    let refused = database
        .count_activity_since(
            &[SIGN_IN_REFUSED, SIGN_IN_HELD_BACK],
            counted_since(seen, "refused_sign_ins", now),
        )
        .await?;
    if refused > 0 {
        points.push(Point::RefusedSignIns { count: refused });
    }
    let unidentified = database.count_awaiting_identification(None).await?;
    if unidentified > seen.get("unidentified").copied().unwrap_or(0) {
        points.push(Point::Unidentified {
            count: unidentified,
        });
    }
    Ok(points)
}

async fn seen_by(state: &AppState, user: UserId) -> Result<HashMap<String, i64>> {
    Ok(state
        .database()
        .attention_seen(user)
        .await?
        .into_iter()
        .collect())
}

/// What this administrator has to look at, the points they marked as seen
/// left out until something new happens.
pub async fn points(state: &AppState, user: UserId) -> Result<Vec<Shown>> {
    let seen = seen_by(state, user).await?;
    let worries = crate::overview::collect(state).await?.worries;
    // A disk seen full that has room again is forgotten as seen, so that
    // filling up again later is told as the news it is.
    for key in seen_with_room(&worries, &seen) {
        state.database().forget_attention_seen(user, key).await?;
    }
    let points = everything(state, worries, &seen, melyxar_core::time::now()).await?;
    Ok(points
        .into_iter()
        .map(|point| Shown {
            state: point.state(),
            may_be_seen: point.key().is_some(),
            point,
        })
        .collect())
}

/// Marks as seen every point shown to this administrator that can be.
pub async fn mark_seen(state: &AppState, user: UserId) -> Result<()> {
    let now = melyxar_core::time::now();
    let seen = seen_by(state, user).await?;
    let worries = crate::overview::collect(state).await?.worries;
    for point in everything(state, worries, &seen, now).await? {
        if let (Some(key), Some(mark)) = (point.key(), point.mark_now(now)) {
            state.database().mark_attention_seen(user, &key, mark).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::{record, Event};
    use melyxar_core::user::Permissions;
    use melyxar_database::Database;

    async fn a_server() -> (tempfile::TempDir, AppState, UserId) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = melyxar_config::Config {
            directories: melyxar_config::Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
                ..Default::default()
            },
            ..melyxar_config::Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");
        let database = Database::open_in_memory().await.expect("database opens");
        let user = database
            .create_user("somebody", None, &Permissions::administrator())
            .await
            .expect("account created");
        (directory, AppState::new(config, database, None, None), user.id)
    }

    fn refused() -> Event {
        Event::SignInRefused {
            name: "nobody".to_string(),
            device: "a browser".to_string(),
        }
    }

    fn of_events(shown: &[Shown]) -> Vec<Point> {
        shown
            .iter()
            .filter(|one| !matches!(one.point, Point::Worry(_)))
            .map(|one| one.point.clone())
            .collect()
    }

    #[test]
    fn a_full_disk_seen_is_quiet_while_it_stays_full_and_forgotten_once_it_has_room() {
        let full = |mount: &str| Worry::DiskNearlyFull {
            mount: mount.to_string(),
            used: 0.97,
        };
        let seen: HashMap<String, i64> = [(disk_key("/mnt/one"), 1), (disk_key("/mnt/gone"), 1)]
            .into_iter()
            .collect();
        let worries = vec![full("/mnt/one"), full("/mnt/two"), Worry::CardUnreachable];

        assert_eq!(
            worries_unseen(worries.clone(), &seen),
            vec![Point::Worry(full("/mnt/two")), Point::Worry(Worry::CardUnreachable)],
            "the disk seen is quiet, the one not seen and the card still speak"
        );
        assert_eq!(
            seen_with_room(&worries, &seen),
            vec![disk_key("/mnt/gone").as_str()],
            "only the disk that has room again is forgotten"
        );
        assert!(Point::Worry(full("/mnt/one")).key().is_some());
        assert!(Point::Worry(Worry::CardUnreachable).key().is_none());
    }

    #[tokio::test]
    async fn refused_sign_ins_are_counted_until_seen_and_speak_again_at_the_next() {
        let (_held, state, user) = a_server().await;
        assert!(of_events(&points(&state, user).await.expect("read")).is_empty());

        record(&state, refused()).await;
        record(&state, refused()).await;
        let shown = points(&state, user).await.expect("read");
        let refusals: Vec<&Shown> = shown
            .iter()
            .filter(|one| matches!(one.point, Point::RefusedSignIns { .. }))
            .collect();
        assert_eq!(refusals.len(), 1);
        assert_eq!(refusals[0].point, Point::RefusedSignIns { count: 2 });
        assert_eq!(refusals[0].state, State::Attention);
        assert!(refusals[0].may_be_seen);

        mark_seen(&state, user).await.expect("marked");
        assert!(of_events(&points(&state, user).await.expect("read")).is_empty());

        // A millisecond is the grain of the mark: the next refusal is later.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        record(&state, refused()).await;
        assert_eq!(
            of_events(&points(&state, user).await.expect("read")),
            vec![Point::RefusedSignIns { count: 1 }]
        );
    }

    #[tokio::test]
    async fn what_one_administrator_saw_is_still_shown_to_another() {
        let (_held, state, user) = a_server().await;
        let other = state
            .database()
            .create_user("another", None, &Permissions::administrator())
            .await
            .expect("account created")
            .id;
        record(&state, refused()).await;
        mark_seen(&state, user).await.expect("marked");
        assert_eq!(
            of_events(&points(&state, other).await.expect("read")),
            vec![Point::RefusedSignIns { count: 1 }]
        );
    }

    #[test]
    fn a_point_about_something_still_wrong_cannot_be_quieted() {
        let now = melyxar_core::time::now();
        for point in [
            Point::Worry(Worry::MediaToolsMissing),
            Point::FallingBehind { count: 1 },
        ] {
            assert!(point.key().is_none());
            assert!(point.mark_now(now).is_none());
        }
        assert_eq!(Point::Unidentified { count: 7 }.mark_now(now), Some(7));
        assert_eq!(Point::FailedTasks { count: 1 }.state(), State::Trouble);
        assert_eq!(Point::Worry(Worry::DiskNearlyFull { mount: "/".into(), used: 0.95 }).state(), State::Attention);
    }

    #[test]
    fn a_point_reads_as_one_flat_object() {
        let worry = Shown {
            point: Point::Worry(Worry::FolderMissing { label: "disk-one".into() }),
            state: State::Trouble,
            may_be_seen: false,
        };
        assert_eq!(
            serde_json::to_value(&worry).expect("written"),
            serde_json::json!({
                "point": "worry",
                "kind": "folder_missing",
                "label": "disk-one",
                "state": "trouble",
                "may_be_seen": false,
            })
        );
        let counted = Shown {
            point: Point::Unidentified { count: 3 },
            state: State::Attention,
            may_be_seen: true,
        };
        assert_eq!(
            serde_json::to_value(&counted).expect("written"),
            serde_json::json!({
                "point": "unidentified",
                "count": 3,
                "state": "attention",
                "may_be_seen": true,
            })
        );
    }

    #[test]
    fn events_count_from_a_day_ago_or_from_when_they_were_seen_if_later() {
        let now = melyxar_core::time::now();
        let mut seen = HashMap::new();
        assert_eq!(counted_since(&seen, "failed_tasks", now), now - A_DAY);
        seen.insert("failed_tasks".to_string(), milliseconds(now - time::Duration::hours(2)));
        let since = counted_since(&seen, "failed_tasks", now);
        assert_eq!(milliseconds(since), milliseconds(now - time::Duration::hours(2)));
        seen.insert("failed_tasks".to_string(), milliseconds(now - time::Duration::days(3)));
        assert_eq!(counted_since(&seen, "failed_tasks", now), now - A_DAY);
    }
}
