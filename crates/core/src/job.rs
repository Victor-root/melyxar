//! Background work: what it is, where it got to, and how it ended.
//!
//! A job is a promise made to someone. The administration screen shows it, a
//! restart has to explain what became of it, and a person who asked for one
//! must be able to stop it. That is why the state lives in the model rather
//! than in whatever task happens to be running.

use crate::id::JobId;
use crate::time::Timestamp;

/// What a job does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    /// Walks the roots of a library and records what changed.
    ScanLibrary,
    /// Looks a work up with a metadata provider. Deliberately separate from a
    /// scan, so it can be replayed on its own and on a subset.
    IdentifyWork,
    FetchImages,
    AnalyseLoudness,
    GenerateThumbnails,
    PurgeActivity,
    Backup,
}

impl JobKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ScanLibrary => "scan_library",
            Self::IdentifyWork => "identify_work",
            Self::FetchImages => "fetch_images",
            Self::AnalyseLoudness => "analyse_loudness",
            Self::GenerateThumbnails => "generate_thumbnails",
            Self::PurgeActivity => "purge_activity",
            Self::Backup => "backup",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "scan_library" => Some(Self::ScanLibrary),
            "identify_work" => Some(Self::IdentifyWork),
            "fetch_images" => Some(Self::FetchImages),
            "analyse_loudness" => Some(Self::AnalyseLoudness),
            "generate_thumbnails" => Some(Self::GenerateThumbnails),
            "purge_activity" => Some(Self::PurgeActivity),
            "backup" => Some(Self::Backup),
            _ => None,
        }
    }
}

/// Where a job stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    /// Whether the job is over, whatever the outcome.
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

/// Priority of a job. Higher runs first, because a request made by a person
/// has to beat a refresh nobody is waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobPriority(i32);

impl JobPriority {
    /// Work nobody asked for, such as a periodic refresh.
    pub const BACKGROUND: Self = Self(0);
    /// Work someone is waiting on, such as a scan started from a button.
    pub const REQUESTED: Self = Self(10);

    pub const fn get(self) -> i32 {
        self.0
    }

    pub const fn from_stored(value: i32) -> Self {
        Self(value)
    }
}

/// A job as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: JobId,
    pub kind: JobKind,
    pub priority: JobPriority,
    pub state: JobState,
    /// What the job is about, such as a library or a work.
    pub target_id: Option<String>,
    pub progress_done: i64,
    /// Unknown until the work has been sized up, which is why it is optional
    /// rather than zero: a progress bar showing nothing is better than one
    /// showing a lie.
    pub progress_total: Option<i64>,
    pub failure_reason: Option<String>,
    pub created_at: Timestamp,
    pub started_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
}

impl Job {
    /// Share of the work done, when the size is known.
    pub fn ratio(&self) -> Option<f64> {
        self.progress_total
            .filter(|total| *total > 0)
            .map(|total| (self.progress_done as f64 / total as f64).clamp(0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_and_state_survives_a_round_trip_through_its_stored_form() {
        for kind in [
            JobKind::ScanLibrary,
            JobKind::IdentifyWork,
            JobKind::FetchImages,
            JobKind::AnalyseLoudness,
            JobKind::GenerateThumbnails,
            JobKind::PurgeActivity,
            JobKind::Backup,
        ] {
            assert_eq!(JobKind::parse(kind.as_str()), Some(kind));
        }
        for state in [
            JobState::Queued,
            JobState::Running,
            JobState::Succeeded,
            JobState::Failed,
            JobState::Cancelled,
        ] {
            assert_eq!(JobState::parse(state.as_str()), Some(state));
        }
    }

    #[test]
    fn a_job_someone_is_waiting_on_outranks_a_refresh_nobody_asked_for() {
        assert!(JobPriority::REQUESTED > JobPriority::BACKGROUND);
    }

    #[test]
    fn only_a_finished_job_counts_as_finished() {
        assert!(!JobState::Queued.is_finished());
        assert!(!JobState::Running.is_finished());
        assert!(JobState::Succeeded.is_finished());
        assert!(JobState::Failed.is_finished());
        assert!(JobState::Cancelled.is_finished());
    }

    fn job(done: i64, total: Option<i64>) -> Job {
        Job {
            id: JobId::new(),
            kind: JobKind::ScanLibrary,
            priority: JobPriority::REQUESTED,
            state: JobState::Running,
            target_id: None,
            progress_done: done,
            progress_total: total,
            failure_reason: None,
            created_at: crate::time::now(),
            started_at: None,
            finished_at: None,
        }
    }

    #[test]
    fn progress_is_unknown_until_the_work_has_been_sized_up() {
        assert_eq!(job(3, None).ratio(), None);
        assert_eq!(job(0, Some(0)).ratio(), None);
    }

    #[test]
    fn progress_never_leaves_its_range_whatever_the_counters_say() {
        assert_eq!(job(5, Some(10)).ratio(), Some(0.5));
        assert_eq!(job(30, Some(10)).ratio(), Some(1.0));
    }
}
