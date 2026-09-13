//! Turning a subtitle track into the one form a browser draws.
//!
//! A browser reads WebVTT and nothing else. Everything else a film carries,
//! SubRip inside the file, an .srt sitting next to it, ASS with its own
//! styling, has to be converted before it can be shown alongside the picture.
//!
//! Converting is cheap: a whole film's subtitles are a few tens of kilobytes
//! of text, and the tool reads only the subtitle stream. What is not cheap is
//! doing it again for every viewer of every film, so the answer is written to
//! the cache and read from there afterwards.
//!
//! Subtitles that are pictures rather than text, the kind a disc carries, are
//! not converted here at all. Turning a picture into words is a different
//! trade, and the playback decision already sends those down another road:
//! they are drawn into the picture instead.

use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;

use tokio::process::Command as TokioCommand;

use crate::{FfmpegError, Result};

/// Builds the conversion of one subtitle track to WebVTT.
///
/// `stream_index` is the track's place inside the file, and is left out for a
/// subtitle that is a file of its own: such a file holds one track, and naming
/// a stream in it would only be a way to get it wrong.
pub fn to_web_vtt_arguments(
    source: &Path,
    destination: &Path,
    stream_index: Option<i32>,
) -> Vec<OsString> {
    let mut arguments = vec![
        OsString::from("-hide_banner"),
        OsString::from("-loglevel"),
        OsString::from("error"),
        OsString::from("-y"),
        OsString::from("-i"),
        source.as_os_str().to_os_string(),
    ];

    if let Some(index) = stream_index {
        arguments.push(OsString::from("-map"));
        arguments.push(OsString::from(format!("0:{index}")));
    }

    arguments.push(OsString::from("-c:s"));
    arguments.push(OsString::from("webvtt"));
    // Nothing but the words: a subtitle track carried alongside a picture and
    // a soundtrack would make the tool read the whole film to write a text
    // file.
    arguments.push(OsString::from("-vn"));
    arguments.push(OsString::from("-an"));
    arguments.push(OsString::from("-f"));
    arguments.push(OsString::from("webvtt"));
    arguments.push(destination.as_os_str().to_os_string());
    arguments
}

/// Writes one subtitle track out as WebVTT.
pub async fn to_web_vtt(
    tool: &Path,
    source: &Path,
    destination: &Path,
    stream_index: Option<i32>,
) -> Result<()> {
    let output = TokioCommand::new(tool)
        .args(to_web_vtt_arguments(source, destination, stream_index))
        .stdin(Stdio::null())
        .output()
        .await?;

    if !output.status.success() {
        return Err(FfmpegError::Failed {
            tool: "ffmpeg",
            status: output.status.to_string(),
            output: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn written(arguments: &[OsString]) -> Vec<String> {
        arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_track_inside_the_film_is_named_by_its_place_in_it() {
        let arguments = written(&to_web_vtt_arguments(
            &PathBuf::from("/films/Quiet.Harbour.2019.mkv"),
            &PathBuf::from("/cache/subtitles/one.vtt"),
            Some(3),
        ));
        let map = arguments
            .iter()
            .position(|value| value == "-map")
            .expect("the track is named");
        assert_eq!(arguments[map + 1], "0:3");
        assert!(arguments.ends_with(&["/cache/subtitles/one.vtt".to_string()]));
    }

    #[test]
    fn a_subtitle_file_of_its_own_is_taken_whole() {
        // It holds one track, so naming a stream in it would only be a way to
        // get it wrong.
        let arguments = written(&to_web_vtt_arguments(
            &PathBuf::from("/films/Quiet.Harbour.2019.fr.srt"),
            &PathBuf::from("/cache/subtitles/one.vtt"),
            None,
        ));
        assert!(!arguments.iter().any(|value| value == "-map"));
    }

    #[test]
    fn nothing_but_the_words_is_read() {
        // A subtitle track carried alongside the picture would make the tool
        // read the whole film to write a text file.
        let arguments = written(&to_web_vtt_arguments(
            &PathBuf::from("/films/Quiet.Harbour.2019.mkv"),
            &PathBuf::from("/cache/subtitles/one.vtt"),
            Some(3),
        ));
        assert!(arguments.iter().any(|value| value == "-vn"));
        assert!(arguments.iter().any(|value| value == "-an"));
    }

    #[test]
    fn the_form_asked_for_is_the_one_a_browser_draws() {
        let arguments = written(&to_web_vtt_arguments(
            &PathBuf::from("/films/Quiet.Harbour.2019.mkv"),
            &PathBuf::from("/cache/subtitles/one.vtt"),
            Some(3),
        ));
        let codec = arguments
            .iter()
            .position(|value| value == "-c:s")
            .expect("a form is asked for");
        assert_eq!(arguments[codec + 1], "webvtt");
        let format = arguments
            .iter()
            .position(|value| value == "-f")
            .expect("the container is stated");
        assert_eq!(
            arguments[format + 1],
            "webvtt",
            "stated rather than guessed from the name, so a cache file with \
             any name still comes out right"
        );
    }

    #[tokio::test]
    async fn a_real_track_comes_out_as_something_a_browser_reads() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let subtitle = directory.path().join("Quiet.Harbour.2019.fr.srt");
        std::fs::write(
            &subtitle,
            "1\n00:00:01,000 --> 00:00:03,500\nBonsoir.\n\n\
             2\n00:00:04,000 --> 00:00:06,000\nLe port est calme.\n",
        )
        .expect("a subtitle file");

        let tools = crate::ToolPaths::discover(None, None).expect("the tools are installed here");
        let destination = directory.path().join("out.vtt");
        to_web_vtt(&tools.ffmpeg, &subtitle, &destination, None)
            .await
            .expect("converted");

        let written = std::fs::read_to_string(&destination).expect("read back");
        assert!(
            written.starts_with("WEBVTT"),
            "a browser refuses anything that does not say so first: {written}"
        );
        assert!(written.contains("Bonsoir."));
        assert!(written.contains("Le port est calme."));
        assert!(
            written.contains("01.000 --> ") && !written.contains("01,000"),
            "WebVTT separates the fraction with a full stop where SubRip uses \
             a comma, and a browser refuses the comma: {written}"
        );
    }

    #[tokio::test]
    async fn a_file_that_holds_no_subtitle_is_refused_rather_than_left_empty() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let nothing = directory.path().join("not-a-film.mkv");
        std::fs::write(&nothing, b"not a film at all").expect("a file");

        let tools = crate::ToolPaths::discover(None, None).expect("the tools are installed here");
        assert!(
            to_web_vtt(
                &tools.ffmpeg,
                &nothing,
                &directory.path().join("out.vtt"),
                None
            )
            .await
            .is_err(),
            "a viewer turning subtitles on must be told, not handed an empty file"
        );
    }
}
