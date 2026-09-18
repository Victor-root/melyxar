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
    /// Reads one file again for what it says about itself.
    ///
    /// A scan only opens a file whose size or date has changed, which is what
    /// keeps a library of thousands from being read through every time. A
    /// server that has learnt to read something new out of a file has no way
    /// to reach the ones it has already described, and this is it.
    ReadCopyAgain,
    FetchImages,
    AnalyseLoudness,
    /// Reads every film of a library for where its picture can be started.
    ///
    /// A job of its own rather than a pass of the scan, because it reads each
    /// file from end to end and a scan has to be over quickly. A library says
    /// whether the scan does it anyway; otherwise this is what the nightly
    /// upkeep starts, and what a button starts when somebody will not wait for
    /// the night.
    ReadKeyFrames,
    /// The same, for the thumbnails somebody drags along the playback bar.
    GenerateThumbnails,
    /// The same, for the subtitles made of words a film carries inside it.
    ///
    /// The words are interleaved with the picture from end to end, so pulling
    /// them out means reading the file through exactly as the other two do.
    /// Asked for when somebody opened the film, that reading fell on the one
    /// person who was waiting.
    PullOutSubtitles,
    PurgeActivity,
    Backup,
}

impl JobKind {
    /// Every kind there is.
    ///
    /// Written down rather than left to whoever needs the list, because
    /// several things have to cover all of them: the interface needs a
    /// sentence for each, and a kind with none reaches the screen as its own
    /// name.
    pub const ALL: [Self; 10] = [
        Self::ScanLibrary,
        Self::IdentifyWork,
        Self::ReadCopyAgain,
        Self::FetchImages,
        Self::AnalyseLoudness,
        Self::ReadKeyFrames,
        Self::GenerateThumbnails,
        Self::PullOutSubtitles,
        Self::PurgeActivity,
        Self::Backup,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ScanLibrary => "scan_library",
            Self::IdentifyWork => "identify_work",
            Self::ReadCopyAgain => "read_copy_again",
            Self::FetchImages => "fetch_images",
            Self::AnalyseLoudness => "analyse_loudness",
            Self::ReadKeyFrames => "read_key_frames",
            Self::GenerateThumbnails => "generate_thumbnails",
            Self::PullOutSubtitles => "pull_out_subtitles",
            Self::PurgeActivity => "purge_activity",
            Self::Backup => "backup",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "scan_library" => Some(Self::ScanLibrary),
            "identify_work" => Some(Self::IdentifyWork),
            "read_copy_again" => Some(Self::ReadCopyAgain),
            "fetch_images" => Some(Self::FetchImages),
            "analyse_loudness" => Some(Self::AnalyseLoudness),
            "read_key_frames" => Some(Self::ReadKeyFrames),
            "generate_thumbnails" => Some(Self::GenerateThumbnails),
            "pull_out_subtitles" => Some(Self::PullOutSubtitles),
            "purge_activity" => Some(Self::PurgeActivity),
            "backup" => Some(Self::Backup),
            _ => None,
        }
    }
}

/// Which part of its work a job is on.
///
/// A scan is several passes end to end, and the long ones are at the end. A
/// bar that fills up, drops back to nothing and sets off again looks exactly
/// like a server that crashed and started over, so the pass says its own name.
///
/// One list for every kind of job rather than one per kind: a step is shown
/// next to the kind, so there is never a question of which list a word is
/// from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStep {
    /// Walking the roots of the library and recording what changed.
    WalkingFolders,
    /// Reading the file names of the works nobody has named yet.
    ReadingNamesAgain,
    /// Asking the analyser what each file holds.
    AnalysingFiles,
    /// Reading each film through for the places its picture can be started.
    ReadingKeyFrames,
    /// Reading each film through for the thumbnails of its playback bar.
    MakingThumbnails,
    /// Reading each film through for the subtitles made of words inside it.
    PullingOutSubtitles,
    /// Asking the metadata provider about the works that are waiting.
    AskingTheProvider,
    /// Asking again about the films that have a name and are missing the rest.
    FillingInWhatIsMissing,
}

impl JobStep {
    /// Every step there is, for the same reason as the kinds above.
    pub const ALL: [Self; 8] = [
        Self::WalkingFolders,
        Self::ReadingNamesAgain,
        Self::AnalysingFiles,
        Self::ReadingKeyFrames,
        Self::MakingThumbnails,
        Self::PullingOutSubtitles,
        Self::AskingTheProvider,
        Self::FillingInWhatIsMissing,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::WalkingFolders => "walking_folders",
            Self::ReadingNamesAgain => "reading_names_again",
            Self::AnalysingFiles => "analysing_files",
            Self::ReadingKeyFrames => "reading_key_frames",
            Self::MakingThumbnails => "making_thumbnails",
            Self::PullingOutSubtitles => "pulling_out_subtitles",
            Self::AskingTheProvider => "asking_the_provider",
            Self::FillingInWhatIsMissing => "filling_in_what_is_missing",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "walking_folders" => Some(Self::WalkingFolders),
            "reading_names_again" => Some(Self::ReadingNamesAgain),
            "analysing_files" => Some(Self::AnalysingFiles),
            "reading_key_frames" => Some(Self::ReadingKeyFrames),
            "making_thumbnails" => Some(Self::MakingThumbnails),
            "pulling_out_subtitles" => Some(Self::PullingOutSubtitles),
            "asking_the_provider" => Some(Self::AskingTheProvider),
            "filling_in_what_is_missing" => Some(Self::FillingInWhatIsMissing),
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
    /// Cut short by a restart. Neither a failure nor a stop somebody asked
    /// for: the work was fine and the server went away under it, which is why
    /// it is the one ending that is taken up again on its own.
    Interrupted,
}

impl JobState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            "interrupted" => Some(Self::Interrupted),
            _ => None,
        }
    }

    /// Whether the job is over, whatever the outcome.
    pub fn is_finished(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }

    /// Whether this ending is one the server takes up again on its own.
    ///
    /// Only a restart. A job that failed would fail again, and a job somebody
    /// stopped was stopped on purpose: starting either back up would be the
    /// server arguing with the person using it.
    pub fn is_worth_taking_up_again(self) -> bool {
        matches!(self, Self::Interrupted)
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

/// How long to wait before trying a job again.
///
/// The wait doubles each time and then stops growing, so a provider that is
/// down is asked again soon at first and rarely after a while. No randomness:
/// on a server with one user there is nothing to spread out, and a delay that
/// can be worked out by hand is a delay that can be explained.
pub fn retry_delay_seconds(attempt: u32) -> u64 {
    const FIRST: u64 = 30;
    const LONGEST: u64 = 6 * 60 * 60;
    FIRST.saturating_mul(1u64 << attempt.min(16)).min(LONGEST)
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
    /// Which pass the job is on, when it has said. The counters below are
    /// counting that pass and nothing else.
    pub step: Option<JobStep>,
    /// What it is on at this very moment, when the pass says so: the name of
    /// one file. The long passes read two at a time, so this is whichever of
    /// them started last.
    pub doing: Option<String>,
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
    fn work_someone_is_waiting_on_comes_before_work_nobody_asked_for() {
        assert!(JobPriority::REQUESTED > JobPriority::BACKGROUND);
        assert_eq!(
            JobPriority::from_stored(JobPriority::REQUESTED.get()),
            JobPriority::REQUESTED,
            "a priority read back from the storage is the one that was written"
        );
        assert_eq!(JobPriority::BACKGROUND.get(), 0);
    }

    #[test]
    fn every_kind_and_state_survives_a_round_trip_through_its_stored_form() {
        for kind in JobKind::ALL {
            assert_eq!(JobKind::parse(kind.as_str()), Some(kind));
        }
        for state in [
            JobState::Queued,
            JobState::Running,
            JobState::Succeeded,
            JobState::Failed,
            JobState::Cancelled,
            JobState::Interrupted,
        ] {
            assert_eq!(JobState::parse(state.as_str()), Some(state));
        }
        for step in [
            JobStep::WalkingFolders,
            JobStep::ReadingNamesAgain,
            JobStep::AnalysingFiles,
            JobStep::ReadingKeyFrames,
            JobStep::MakingThumbnails,
            JobStep::AskingTheProvider,
            JobStep::FillingInWhatIsMissing,
        ] {
            assert_eq!(JobStep::parse(step.as_str()), Some(step));
        }
        assert_eq!(JobStep::parse("something_new"), None);
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
        assert!(JobState::Interrupted.is_finished());
    }

    #[test]
    fn only_a_job_a_restart_cut_short_is_taken_up_again() {
        // A job that failed would fail again, and one somebody stopped was
        // stopped on purpose: starting either back up would be the server
        // arguing with the person using it.
        assert!(JobState::Interrupted.is_worth_taking_up_again());
        assert!(!JobState::Failed.is_worth_taking_up_again());
        assert!(!JobState::Cancelled.is_worth_taking_up_again());
        assert!(!JobState::Succeeded.is_worth_taking_up_again());
    }

    fn job(done: i64, total: Option<i64>) -> Job {
        Job {
            id: JobId::new(),
            kind: JobKind::ScanLibrary,
            priority: JobPriority::REQUESTED,
            state: JobState::Running,
            target_id: None,
            step: None,
            doing: None,
            progress_done: done,
            progress_total: total,
            failure_reason: None,
            created_at: crate::time::now(),
            started_at: None,
            finished_at: None,
        }
    }

    #[test]
    fn the_wait_before_another_try_grows_and_then_stops_growing() {
        assert_eq!(retry_delay_seconds(0), 30);
        assert_eq!(retry_delay_seconds(1), 60);
        assert_eq!(retry_delay_seconds(2), 120);

        let longest = retry_delay_seconds(60);
        assert_eq!(longest, 6 * 60 * 60);
        assert_eq!(
            retry_delay_seconds(20),
            longest,
            "a wait that keeps doubling ends up never trying again at all"
        );
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
