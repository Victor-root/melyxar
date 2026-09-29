//! A song converted while it is listened to.
//!
//! Most songs reach a browser as they lie on the disk. The few forms no
//! browser plays, Apple's lossless, WMA, Monkey's Audio and their like, are
//! turned into a plain file of sound as they are sent: no session, no
//! segments, nothing kept on the disk, the tool writing straight into the
//! answer. That is the whole of what music asks of this crate, kept apart from
//! how films are rebuilt (see `docs/architecture/06-musique.md`).
//!
//! A song converted this way cannot be moved about in by the browser, which
//! has no length to go by: a move is a new request starting where it lands.

use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;

use melyxar_core::time::Millis;
use tokio::process::{Child, ChildStdout, Command as TokioCommand};

use crate::{FfmpegError, Result};

/// What a song is converted into, chosen by what the browser says it plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Converted {
    /// Opus in Ogg: the better sound for its weight, for a browser that has it.
    Opus,
    /// MP3, which every browser plays.
    Mp3,
}

impl Converted {
    /// What the answer is said to be.
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Opus => "audio/ogg",
            Self::Mp3 => "audio/mpeg",
        }
    }

    /// How heavy it is made, in kilobits a second, when nothing asks for
    /// lighter: as good as either form gets to the ear.
    pub fn usual_kbps(self) -> u32 {
        match self {
            Self::Opus => 192,
            Self::Mp3 => 320,
        }
    }
}

/// A song on its way, and the tool writing it. The tool stops when this is
/// dropped, which is what a listener going away amounts to.
pub struct ConvertedSong {
    pub output: ChildStdout,
    /// Held for as long as the song is being sent.
    pub process: Child,
}

/// Starts converting a song, from `start` on, at `kbps` kilobits a second.
pub fn convert(
    tool: &Path,
    song: &Path,
    start: Millis,
    into: Converted,
    kbps: u32,
) -> Result<ConvertedSong> {
    let mut process = TokioCommand::new(tool)
        .args(arguments(song, start, into, kbps))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let output = process.stdout.take().ok_or_else(|| {
        FfmpegError::Spawn(std::io::Error::other("the tool gave no output to read"))
    })?;
    Ok(ConvertedSong { output, process })
}

/// What the tool is told, written as a value so it can be read without
/// running anything.
pub fn arguments(song: &Path, start: Millis, into: Converted, kbps: u32) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-nostdin"]
        .into_iter()
        .map(OsString::from)
        .collect();
    if start.get() > 0 {
        arguments.push("-ss".into());
        arguments.push(format!("{:.3}", start.as_seconds_f64()).into());
    }
    arguments.push("-i".into());
    arguments.push(song.as_os_str().to_os_string());
    // The sound alone: a cover carried inside is a picture, and a picture in
    // the answer would make it a video to the browser.
    arguments.extend(["-map", "0:a:0", "-vn", "-map_metadata", "-1"].map(OsString::from));
    let (codec, format) = match into {
        Converted::Opus => ("libopus", "ogg"),
        Converted::Mp3 => ("libmp3lame", "mp3"),
    };
    let rate = format!("{kbps}k");
    arguments.extend(["-c:a", codec, "-b:a", &rate, "-f", format, "pipe:1"].map(OsString::from));
    arguments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(arguments: &[OsString]) -> String {
        arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn a_song_is_converted_to_its_sound_alone_from_where_it_is_asked() {
        let from_start = written(&arguments(
            Path::new("/m/a.m4a"),
            Millis::ZERO,
            Converted::Opus,
            Converted::Opus.usual_kbps(),
        ));
        assert_eq!(
            from_start,
            "-hide_banner -loglevel error -nostdin -i /m/a.m4a -map 0:a:0 -vn -map_metadata -1 \
             -c:a libopus -b:a 192k -f ogg pipe:1"
        );
        let later = written(&arguments(
            Path::new("/m/a.wma"),
            Millis::new(61_500),
            Converted::Mp3,
            128,
        ));
        assert!(later.contains("-ss 61.500 -i /m/a.wma"), "{later}");
        assert!(
            later.ends_with("-c:a libmp3lame -b:a 128k -f mp3 pipe:1"),
            "{later}"
        );
    }

    #[tokio::test]
    async fn a_song_the_browser_cannot_play_comes_out_as_one_it_can() {
        use tokio::io::AsyncReadExt;

        let Ok(tools) = crate::ToolPaths::discover(None, None) else {
            eprintln!("no media tool here, nothing was converted");
            return;
        };
        let directory = tempfile::tempdir().expect("temporary directory");
        let song = directory.path().join("Quiet Harbour.m4a");
        let made = TokioCommand::new(&tools.ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
            .arg("sine=frequency=440:duration=3")
            .args(["-c:a", "alac"])
            .arg(&song)
            .status()
            .await
            .expect("the tool runs");
        assert!(made.success());

        for (into, magic) in [
            (Converted::Opus, &b"OggS"[..]),
            (Converted::Mp3, &b"ID3"[..]),
        ] {
            let mut converted = convert(
                &tools.ffmpeg,
                &song,
                Millis::new(1_000),
                into,
                into.usual_kbps(),
            )
            .expect("started");
            let mut sound = Vec::new();
            converted
                .output
                .read_to_end(&mut sound)
                .await
                .expect("read");
            assert!(sound.len() > 1_000, "{into:?}: {} bytes", sound.len());
            assert!(
                sound.starts_with(magic) || (into == Converted::Mp3 && sound[0] == 0xff),
                "{into:?} starts as its form does"
            );
        }
    }
}
