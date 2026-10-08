//! Writing subtitles by listening: the sound of a video through a speech model.
//!
//! Two programs in a row. The encoder takes the first sound track out of the
//! video as a plain mono recording at the one rate the models are made for, and
//! the speech tool (whisper.cpp) turns that recording into subtitle lines and
//! says which language it heard. Everything is written into a folder of its own
//! that is removed whatever happens, so a reading called off halfway leaves
//! nothing behind.
//!
//! The tool is a program of its own, installed beside the server by the
//! installation script, and found here the way the encoder is.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use tokio::process::Command as TokioCommand;

use crate::process::{output_of, output_reporting, AskedToStop};
use crate::spoken_lines::{loop_in, subrip_of};
use crate::{find_on_path, FfmpegError, Result};

/// Where the installation script puts the tool, which is where it is looked
/// for when the configuration does not say otherwise.
pub const INSTALLED_AT: &str = "/opt/melyxar/whisper/whisper-cli";

/// The rate every model is made for. Anything else is resampled by the tool
/// itself at a cost, so the recording is made at this one.
const SAMPLE_RATE: &str = "16000";

/// The speech tool, found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechTool {
    path: PathBuf,
}

impl SpeechTool {
    /// Finds the tool: where the configuration says, else on the search path,
    /// else where the installation script puts it. Nothing when it is not
    /// there: the server runs the same without it, minus these subtitles.
    pub fn discover(configured: Option<&Path>) -> Option<Self> {
        let path = match configured {
            Some(path) => path.is_file().then(|| path.to_path_buf()),
            None => find_on_path("whisper-cli").or_else(|| {
                let installed = PathBuf::from(INSTALLED_AT);
                installed.is_file().then_some(installed)
            }),
        }?;
        Some(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// What does the listening: the tool, the model it is given, and how many
/// threads of the processor it may take.
#[derive(Debug, Clone, Copy)]
pub struct Listener<'a> {
    pub tool: &'a SpeechTool,
    pub model: &'a Path,
    pub threads: usize,
}

/// What listening to a video gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heard {
    /// The subtitle file, in the SubRip form every other subtitle is kept in.
    pub subrip: String,
    /// The language the model heard, as the two letters it names it by.
    pub language: Option<String>,
    /// How many lines of subtitle there are. Nought for a video with nothing
    /// said in it.
    pub lines: usize,
}

/// A folder removed when it goes out of scope, which is also when a reading
/// called off drops the future holding it.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Listens to the first sound track of a video and writes what it hears.
///
/// `scratch` is a folder to work in, in the cache: the recording of a long
/// video is a few hundred megabytes. The listener's threads bound the
/// processor taken, and the tool is run at the lowest priority there is,
/// since this is work nobody is waiting for and the machine may be playing
/// something. `on_progress` is told how far the tool has got, from nought to
/// one, as it listens.
pub async fn listen(
    tools: &crate::ToolPaths,
    listener: Listener<'_>,
    video: &Path,
    scratch: &Path,
    asked_to_stop: AskedToStop,
    on_progress: impl Fn(f64),
) -> Result<Heard> {
    let folder = scratch.join(format!("listening-{}", unique_suffix()));
    std::fs::create_dir_all(&folder)?;
    let folder = Scratch(folder);
    let recording = folder.0.join("sound.wav");
    let written = folder.0.join("heard");

    let mut recording_of_it = TokioCommand::new(&tools.ffmpeg);
    let mut args = recording_arguments(video, &recording);
    crate::formats::guard_the_one_input(&mut args, tools.allowed_formats());
    recording_of_it.args(args);
    let output = output_of(recording_of_it, asked_to_stop.clone()).await?;
    if !output.status.success() {
        return Err(FfmpegError::from_output("ffmpeg", &output));
    }

    let mut listening = low_priority(listener.tool.path());
    let arguments = listening_arguments(listener.model, &recording, &written, listener.threads);
    tracing::debug!(video = %video.display(), arguments = ?arguments, "the speech tool is being set going");
    listening.args(arguments);
    let started = std::time::Instant::now();
    let output = output_reporting(listening, asked_to_stop, |line| match progress_in(line) {
        Some(fraction) => {
            on_progress(fraction);
            true
        }
        None => false,
    })
    .await?;
    if !output.status.success() {
        return Err(FfmpegError::from_output("whisper-cli", &output));
    }

    let report = std::fs::read_to_string(written.with_extension("json"))?;
    if let Some(stuck) = loop_in(&report) {
        tracing::warn!(
            video = %video.display(),
            from_second = stuck.from_ms / 1_000,
            to_second = stuck.to_ms / 1_000,
            blocks = stuck.blocks,
            text = stuck.text,
            "the speech tool repeated itself over a stretch of the video"
        );
    }
    let subrip = subrip_of(&report)
        .ok_or_else(|| FfmpegError::MalformedReport("the report of what was heard".to_string()))?;
    let language = language_of(&report);
    let lines = lines_in(&subrip);
    tracing::debug!(
        video = %video.display(),
        lines,
        language = language.as_deref(),
        took_seconds = started.elapsed().as_secs(),
        "the speech tool heard a video"
    );
    Ok(Heard {
        lines,
        subrip,
        language,
    })
}

/// A name no other reading of the same moment can have.
fn unique_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!("{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))
}

/// The tool, run through `nice` where there is one, so that listening gives
/// way to everything else the machine is asked to do.
fn low_priority(tool: &Path) -> TokioCommand {
    match find_on_path("nice") {
        Some(nice) => {
            let mut command = TokioCommand::new(nice);
            command.args(["-n", "19"]).arg(tool);
            command
        }
        None => TokioCommand::new(tool),
    }
}

/// What the encoder is told: the first sound track, as mono at the rate the
/// models are made for, written as plain samples.
pub fn recording_arguments(video: &Path, recording: &Path) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = ["-hide_banner", "-nostats", "-nostdin", "-loglevel", "error"]
        .map(OsString::from)
        .to_vec();
    // The whole sound is written to the disk before it is listened to.
    arguments.extend(crate::reading::no_further_than_believable());
    arguments.push(OsString::from("-i"));
    arguments.push(video.as_os_str().to_os_string());
    arguments.extend(
        ["-map", "0:a:0", "-vn", "-ac", "1", "-ar", SAMPLE_RATE, "-c:a", "pcm_s16le", "-y"]
            .map(OsString::from),
    );
    arguments.push(recording.as_os_str().to_os_string());
    arguments
}

/// How far along the tool says it is, from nought to one, when this line of
/// its error output is one of the lines it reports that in.
fn progress_in(line: &str) -> Option<f64> {
    let (_, after) = line.split_once("progress = ")?;
    let percent: f64 = after.trim().strip_suffix('%')?.trim().parse().ok()?;
    Some((percent / 100.0).clamp(0.0, 1.0))
}

/// What the speech tool is told: the model, the recording, the language left
/// for it to find, and the report it is to write: the lines are made from the
/// blocks of that report, not by the tool. It is told too to carry no text
/// from one stretch of the recording to the next: left to do so, a single
/// stretch where it repeats itself is read back as what is being said and
/// goes on being repeated until the end of the video.
pub fn listening_arguments(model: &Path, recording: &Path, written: &Path, threads: usize) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = vec!["-m".into(), model.as_os_str().to_os_string()];
    arguments.extend(["-f".into(), recording.as_os_str().to_os_string()]);
    arguments.extend(["-l".into(), "auto".into(), "-t".into(), threads.max(1).to_string().into()]);
    arguments.extend(["-mc".into(), "0".into()]);
    arguments.extend(["-np".into(), "-pp".into(), "-oj".into()]);
    arguments.extend(["-of".into(), written.as_os_str().to_os_string()]);
    arguments
}

/// The language the report says was heard.
fn language_of(report: &str) -> Option<String> {
    let report: serde_json::Value = serde_json::from_str(report).ok()?;
    let language = report.get("result")?.get("language")?.as_str()?;
    (!language.is_empty()).then(|| language.to_string())
}

/// How many lines of subtitle a SubRip file holds: one for each timing line.
fn lines_in(subrip: &str) -> usize {
    subrip.lines().filter(|line| line.contains(" --> ")).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_recording_is_mono_at_the_rate_of_the_models() {
        let arguments: Vec<String> = recording_arguments(Path::new("/media/clip.mkv"), Path::new("/tmp/sound.wav"))
            .into_iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        let joined = arguments.join(" ");
        assert!(joined.contains("-i /media/clip.mkv"));
        assert!(joined.contains("-map 0:a:0"));
        assert!(joined.contains("-ac 1 -ar 16000"));
        assert!(joined.ends_with("-y /tmp/sound.wav"));
    }

    #[test]
    fn the_language_is_left_to_the_model_and_a_subtitle_file_is_asked_for() {
        let arguments: Vec<String> = listening_arguments(
            Path::new("/data/model.bin"),
            Path::new("/tmp/sound.wav"),
            Path::new("/tmp/heard"),
            0,
        )
        .into_iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
        let joined = arguments.join(" ");
        assert!(joined.contains("-m /data/model.bin"));
        assert!(joined.contains("-l auto"));
        assert!(joined.contains("-mc 0"), "what was heard is not fed back to the model");
        assert!(joined.contains("-oj"), "the report carries what was heard");
        assert!(joined.contains("-pp"), "how far it has got is asked for");
        assert!(!joined.contains("-osrt"), "the lines are made from the report");
        assert!(!joined.contains("-ml"), "the times of single words are not trusted");
        assert!(joined.contains("-of /tmp/heard"));
        assert!(joined.contains("-t 1"), "at least one thread, whatever was asked");
    }

    #[test]
    fn the_language_heard_is_read_from_the_report() {
        assert_eq!(
            language_of(r#"{"systeminfo":"x","result":{"language":"fr"},"transcription":[]}"#).as_deref(),
            Some("fr")
        );
        assert_eq!(language_of(r#"{"result":{"language":""}}"#), None);
        assert_eq!(language_of(r#"{"result":{}}"#), None);
        assert_eq!(language_of("not a report"), None);
    }

    #[test]
    fn how_far_the_tool_has_got_is_read_from_its_own_words() {
        assert_eq!(progress_in("whisper_print_progress_callback: progress =  45%"), Some(0.45));
        assert_eq!(progress_in("whisper_print_progress_callback: progress = 100%"), Some(1.0));
        assert_eq!(progress_in("whisper_print_progress_callback: progress =   5%"), Some(0.05));
        assert_eq!(progress_in("whisper_init_from_file: loading model"), None);
        assert_eq!(progress_in("progress = soon"), None);
    }

    #[test]
    fn the_lines_of_a_subtitle_file_are_counted_by_their_timings() {
        let subrip = "1\n00:00:00,000 --> 00:00:02,000\n Hello.\n\n2\n00:00:02,000 --> 00:00:04,500\n Bye.\n\n";
        assert_eq!(lines_in(subrip), 2);
        assert_eq!(lines_in(""), 0);
    }

    #[test]
    fn a_tool_that_is_not_there_is_not_found() {
        assert_eq!(SpeechTool::discover(Some(Path::new("/nowhere/whisper-cli"))), None);
    }

    #[test]
    fn the_scratch_folder_goes_with_the_reading() {
        let parent = tempfile::tempdir().expect("temporary directory");
        let folder = parent.path().join("listening-test");
        std::fs::create_dir_all(&folder).expect("made");
        std::fs::write(folder.join("sound.wav"), b"x").expect("written");
        drop(Scratch(folder.clone()));
        assert!(!folder.exists());
    }
}
