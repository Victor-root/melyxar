//! Starting and supervising a media tool process.
//!
//! Four mechanisms keep processes from piling up, and all four are needed
//! because each covers a case the others miss:
//!
//! 1. the child is killed when its handle is dropped, which covers a task that
//!    unwinds or a session dropped mid-flight;
//! 2. stopping asks politely first and only then insists, so that a partly
//!    written segment is closed rather than truncated;
//! 3. the caller can wait for the process with a deadline, so a tool that
//!    hangs does not hold a session open for ever;
//! 4. sweeping the working directory at start-up removes whatever survived a
//!    crash, since nothing there can belong to a live session.
//!
//! A browser tab that closes never tells the server anything, so none of this
//! is optional.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use melyxar_core::time::Millis;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command as TokioCommand};
use tokio::sync::mpsc;

use crate::command::Command;
use crate::{FfmpegError, Result};

/// How long a process is given to stop on its own before it is forced.
///
/// Long enough to finish writing a segment, short enough that a viewer who
/// jumped elsewhere is not left waiting.
const GRACE_PERIOD: Duration = Duration::from_secs(5);

/// How far along the tool reports being.
///
/// Read from the machine-facing progress stream rather than from the status
/// line meant for humans, which changes wording between versions.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Progress {
    /// Position reached in the output.
    pub position: Millis,
    /// Frames written so far.
    pub frames: Option<i64>,
    /// Pictures a second the tool says it is writing just now.
    ///
    /// The rate of the moment rather than an average: what somebody watching a
    /// film asks is whether the machine is keeping up right now, and a mean
    /// taken over a whole run answers that several minutes late.
    pub pictures_a_second: Option<f64>,
    /// Speed relative to real time. Below one means the machine cannot keep
    /// up, which is what turns into stuttering for the viewer.
    pub speed: Option<f64>,
    /// Set on the final report.
    pub finished: bool,
}

/// A running tool process.
pub struct RunningProcess {
    child: Child,
    tool: &'static str,
}

impl RunningProcess {
    /// Starts a command, optionally forwarding progress reports to a channel.
    ///
    /// The channel is bounded and reports are dropped rather than queued when
    /// nobody is keeping up: progress is a courtesy, and it must never slow
    /// down the process producing the stream someone is watching.
    pub fn start(
        tool: &Path,
        command: &Command,
        progress: Option<mpsc::Sender<Progress>>,
    ) -> Result<Self> {
        let mut builder = TokioCommand::new(tool);
        builder
            .args(command.to_arguments())
            .stdin(Stdio::null())
            .stdout(if progress.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stderr(Stdio::piped())
            // Covers the case nothing else does: a task that unwinds, or a
            // session dropped without anyone calling stop.
            .kill_on_drop(true);

        let mut child = builder.spawn()?;

        if let (Some(sender), Some(stdout)) = (progress, child.stdout.take()) {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                let mut current = Progress::default();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(report) = absorb_progress_line(&mut current, &line) {
                        // Dropping a report is deliberate: a slow listener
                        // must never hold back the process.
                        if sender.try_send(report).is_err() && sender.is_closed() {
                            break;
                        }
                    }
                }
            });
        }

        Ok(Self {
            child,
            tool: "encoder",
        })
    }

    /// Process identifier, while it is still running.
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// Whether the process has already exited.
    pub fn has_exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    /// Asks the process to stop, then forces it if it does not.
    ///
    /// Asking first matters: forced immediately, the tool can leave a segment
    /// half written, and a truncated segment served to a browser breaks
    /// playback in a way that is tedious to diagnose.
    pub async fn stop(mut self) -> Result<()> {
        let Some(pid) = self.child.id() else {
            // Already gone.
            return Ok(());
        };

        request_termination(pid);

        match tokio::time::timeout(GRACE_PERIOD, self.child.wait()).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) => Err(FfmpegError::Spawn(error)),
            Err(_) => {
                tracing::warn!(
                    pid,
                    "the media tool ignored the request to stop, forcing it"
                );
                self.child.kill().await?;
                Ok(())
            }
        }
    }

    /// Stops the process where it stands, without asking first.
    ///
    /// For the one case where politeness buys nothing: a viewer has jumped
    /// somewhere else, so everything this process was doing is worthless, and
    /// waiting the best part of a second for it to finish writing a segment
    /// nobody will watch is that second taken from the viewer. What it leaves
    /// half written is the caller's to clear away, which costs a file removal
    /// rather than a wait.
    pub async fn stop_now(mut self) -> Result<()> {
        self.child.kill().await?;
        Ok(())
    }

    /// Waits for the process and fails when it exited badly.
    ///
    /// The tail of the error output travels with the failure, because a
    /// message naming what went wrong is the difference between a fix and a
    /// debugging session.
    pub async fn wait(mut self) -> Result<()> {
        let stderr = self.child.stderr.take();
        let status = self.child.wait().await?;

        if status.success() {
            return Ok(());
        }

        let output = match stderr {
            Some(stream) => {
                let mut lines = BufReader::new(stream).lines();
                let mut collected = Vec::new();
                while let Ok(Some(line)) = lines.next_line().await {
                    collected.push(line);
                    if collected.len() > 20 {
                        collected.remove(0);
                    }
                }
                collected.join(" | ")
            }
            None => String::new(),
        };

        Err(FfmpegError::Failed {
            tool: self.tool,
            status: status.to_string(),
            output,
        })
    }
}

/// Asks a process to stop the polite way.
///
/// Implemented by handing the work to the system's own tool rather than
/// calling the interface directly, which would require stepping outside the
/// safety guarantees this crate holds to. The cost is one short-lived process
/// on a path taken once per session teardown, which is not a hot path.
fn request_termination(pid: u32) {
    let _ = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Folds one line of the progress stream into the running report.
///
/// The stream is a series of key and value lines ending with a marker, so a
/// report is only emitted once a block is complete.
fn absorb_progress_line(current: &mut Progress, line: &str) -> Option<Progress> {
    let (key, value) = line.split_once('=')?;
    match key.trim() {
        "out_time_us" | "out_time_ms" => {
            // Despite its name the field is in microseconds in both spellings.
            if let Ok(microseconds) = value.trim().parse::<i64>() {
                current.position = Millis::new(microseconds / 1000);
            }
        }
        "frame" => current.frames = value.trim().parse().ok(),
        // Kept only when it reads as a number. Both of these answer `N/A`
        // before the tool has produced anything and again whenever it stalls,
        // and read on a panel somebody is watching, a rate that blinked out
        // every few seconds would look like the machine stopping. A picture
        // merely copied is never drawn, so the tool gives no rate for it at
        // all: that one stays at nothing, which is the truth about it.
        "fps" => {
            if let Ok(rate) = value.trim().parse() {
                current.pictures_a_second = Some(rate);
            }
        }
        "speed" => {
            if let Ok(rate) = value.trim().trim_end_matches('x').parse() {
                current.speed = Some(rate);
            }
        }
        "progress" => {
            current.finished = value.trim() == "end";
            return Some(*current);
        }
        _ => {}
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{AudioOutput, Input, Output, VideoOutput, WhereToCut};
    use crate::ToolPaths;

    #[test]
    fn a_progress_block_is_only_reported_once_it_is_complete() {
        let mut current = Progress::default();
        assert!(absorb_progress_line(&mut current, "frame=120").is_none());
        assert!(absorb_progress_line(&mut current, "out_time_us=5000000").is_none());
        assert!(absorb_progress_line(&mut current, "fps=59.94").is_none());
        assert!(absorb_progress_line(&mut current, "speed=2.5x").is_none());

        let report = absorb_progress_line(&mut current, "progress=continue")
            .expect("the marker completes a block");
        assert_eq!(report.position, Millis::new(5000));
        assert_eq!(report.frames, Some(120));
        assert_eq!(report.pictures_a_second, Some(59.94));
        assert_eq!(report.speed, Some(2.5));
        assert!(!report.finished);
    }

    #[test]
    fn the_final_block_is_marked_as_such() {
        let mut current = Progress::default();
        let report =
            absorb_progress_line(&mut current, "progress=end").expect("the marker completes");
        assert!(report.finished);
    }

    #[test]
    fn an_unreadable_line_is_ignored_rather_than_breaking_the_stream() {
        let mut current = Progress::default();
        assert!(absorb_progress_line(&mut current, "nonsense").is_none());
        assert!(absorb_progress_line(&mut current, "speed=N/A").is_none());
        assert_eq!(current.speed, None);
    }

    #[test]
    fn a_rate_the_tool_cannot_give_yet_leaves_the_last_one_it_could() {
        // Both of these are words rather than numbers until the tool has
        // produced something, and they go back to being words whenever it
        // stalls. Read on a panel a viewer is watching, a rate that blinked
        // out every few seconds would read as the machine stopping.
        let mut current = Progress::default();
        absorb_progress_line(&mut current, "fps=24.0");
        absorb_progress_line(&mut current, "speed=1.6x");
        absorb_progress_line(&mut current, "fps=N/A");
        absorb_progress_line(&mut current, "speed=N/A");

        assert_eq!(current.pictures_a_second, Some(24.0));
        assert_eq!(current.speed, Some(1.6));
    }

    /// Builds a short synthetic clip, so tests never need real content.
    async fn make_clip(path: &Path, seconds: u32) {
        let status = TokioCommand::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                &format!("testsrc=duration={seconds}:size=320x240:rate=10"),
                "-f",
                "lavfi",
                "-i",
                &format!("sine=frequency=440:duration={seconds}"),
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-c:a",
                "aac",
                "-shortest",
                "-y",
            ])
            .arg(path)
            .status()
            .await
            .expect("the tool runs");
        assert!(status.success(), "the sample clip was produced");
    }

    #[tokio::test]
    async fn a_command_runs_to_completion_and_produces_its_output() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        make_clip(&source, 1).await;

        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let destination = directory.path().join("copy.mp4");
        let command = Command::new(Input::new(&source), Output::File(destination.clone()));

        let process =
            RunningProcess::start(&tools.ffmpeg, &command, None).expect("the process starts");
        process.wait().await.expect("the process succeeds");

        assert!(destination.exists(), "the output file was produced");
        assert!(
            std::fs::metadata(&destination).expect("readable").len() > 0,
            "the output file is not empty"
        );
    }

    #[tokio::test]
    async fn a_failing_command_carries_the_reason_rather_than_a_bare_status() {
        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let command = Command::new(
            Input::new("/nowhere/missing.mkv"),
            Output::File(std::path::PathBuf::from("/tmp/never-written.mp4")),
        );

        let process =
            RunningProcess::start(&tools.ffmpeg, &command, None).expect("the process starts");
        let error = process.wait().await.expect_err("a missing input must fail");

        match error {
            FfmpegError::Failed { output, .. } => {
                assert!(
                    !output.is_empty(),
                    "the failure must say what went wrong, not just that it did"
                );
            }
            other => panic!("expected a failure carrying its output, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn progress_is_reported_while_the_tool_works() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        make_clip(&source, 3).await;

        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let command = Command::new(
            Input::new(&source),
            Output::File(directory.path().join("out.mp4")),
        )
        .with_video(VideoOutput::Encode(
            crate::command::VideoEncode::software_h264(),
        ))
        .with_audio(AudioOutput::Copy)
        .reporting_progress();

        let (sender, mut receiver) = mpsc::channel(16);
        let process = RunningProcess::start(&tools.ffmpeg, &command, Some(sender))
            .expect("the process starts");

        let mut reports = Vec::new();
        let collector = tokio::spawn(async move {
            while let Some(report) = receiver.recv().await {
                reports.push(report);
            }
            reports
        });

        process.wait().await.expect("the process succeeds");
        let reports = collector.await.expect("the collector finishes");

        assert!(!reports.is_empty(), "at least one report must arrive");
        assert!(
            reports.last().expect("a last report").finished,
            "the stream must end with a final report"
        );
        assert!(
            reports.iter().any(|report| report.position.get() > 0),
            "a report must show real progress"
        );
    }

    #[tokio::test]
    async fn stopping_a_running_process_ends_it_rather_than_leaving_it_behind() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        make_clip(&source, 30).await;

        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        // Slow preset on purpose, so the process is still working when asked
        // to stop.
        let mut encode = crate::command::VideoEncode::software_h264();
        encode.how = crate::command::Rebuilding::InSoftware {
            quality: 18,
            preset: "veryslow".to_string(),
        };
        let command = Command::new(
            Input::new(&source),
            Output::File(directory.path().join("out.mp4")),
        )
        .with_video(VideoOutput::Encode(encode));

        let mut process =
            RunningProcess::start(&tools.ffmpeg, &command, None).expect("the process starts");
        let pid = process.id().expect("the process has an identifier");
        assert!(!process.has_exited());

        process.stop().await.expect("stopping succeeds");

        // The identifier must no longer name a live process.
        let still_alive = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("the check runs")
            .success();
        assert!(!still_alive, "no process may survive being stopped");
    }

    #[tokio::test]
    async fn segments_are_produced_as_numbered_files_starting_where_asked() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        make_clip(&source, 8).await;

        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let session = directory.path().join("session");
        std::fs::create_dir_all(&session).expect("session directory");

        let command = Command::new(
            Input::new(&source),
            Output::Segments {
                pattern: session.join("segment-%05d.m4s"),
                initialisation: session.join("init.mp4"),
                tool_playlist: session.join("tool.m3u8"),
                cut: WhereToCut::Every(Millis::new(2000)),
                start_number: 7,
            },
        )
        .with_video(VideoOutput::Copy)
        .with_audio(AudioOutput::Copy);

        let process =
            RunningProcess::start(&tools.ffmpeg, &command, None).expect("the process starts");
        process.wait().await.expect("the process succeeds");

        let mut produced: Vec<String> = std::fs::read_dir(&session)
            .expect("the session directory is readable")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".m4s"))
            .collect();
        produced.sort();

        assert!(!produced.is_empty(), "segments were produced");
        assert!(
            produced[0].contains("00007"),
            "numbering must start where asked, got {produced:?}"
        );
        assert!(
            session.join("init.mp4").exists(),
            "the header every segment needs is written beside them, or a player \
             has nothing to put the segments together with"
        );
        assert!(
            std::fs::metadata(session.join("init.mp4"))
                .expect("the header is there")
                .len()
                > 0
        );
    }
}
