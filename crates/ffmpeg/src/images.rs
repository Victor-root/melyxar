//! Turning a picture into what the interface actually serves.
//!
//! The tool that already handles every other frame handles these too, which
//! keeps one binary responsible for pixels instead of two, and keeps the
//! conversion to standard range in the same hands as everywhere else.
//!
//! Sizes are fixed and generated once, on the cold path. A server that resizes
//! on demand spends its afternoon resizing the same poster.

use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;

use tokio::process::Command as TokioCommand;

use crate::{FfmpegError, Result};

/// Quality of the pictures written out.
///
/// Eighty is where the difference stops being visible on a poster and the file
/// stops shrinking usefully.
const QUALITY: u8 = 80;

/// Widths generated for every picture, in pixels.
///
/// One for a card in a grid, one for a grid on a large screen, one for the
/// header of a detail page. A viewer's browser picks among them; the server
/// never resizes on demand.
pub const POSTER_WIDTHS: [u32; 3] = [200, 400, 800];
/// Backdrops are shown wide, so they start where posters end.
pub const BACKDROP_WIDTHS: [u32; 3] = [640, 1280, 1920];
/// A face is shown in a small round frame, and a screen with fine pixels wants
/// twice what it measures. Two widths cover both and no more.
pub const PHOTO_WIDTHS: [u32; 2] = [96, 192];

/// Builds the conversion of one picture to one width.
///
/// The height follows the width so nothing is ever stretched, and an odd
/// number of pixels is allowed here: unlike video, a picture has no encoder
/// demanding even sides.
pub fn resize_arguments(source: &Path, destination: &Path, width: u32) -> Vec<OsString> {
    vec![
        OsString::from("-hide_banner"),
        OsString::from("-loglevel"),
        OsString::from("error"),
        OsString::from("-y"),
        OsString::from("-i"),
        source.as_os_str().to_os_string(),
        OsString::from("-vf"),
        OsString::from(format!("scale={width}:-1:flags=lanczos")),
        OsString::from("-frames:v"),
        OsString::from("1"),
        OsString::from("-c:v"),
        OsString::from("libwebp"),
        OsString::from("-quality"),
        OsString::from(QUALITY.to_string()),
        destination.as_os_str().to_os_string(),
    ]
}

/// Builds the reading of a picture's average colour.
///
/// The picture is squeezed down to a single pixel and that pixel is read. It
/// is the average rather than the most frequent colour, which is what a card
/// wants: a background that sits under the artwork without fighting it.
pub fn average_colour_arguments(source: &Path) -> Vec<OsString> {
    vec![
        OsString::from("-hide_banner"),
        OsString::from("-loglevel"),
        OsString::from("error"),
        OsString::from("-i"),
        source.as_os_str().to_os_string(),
        OsString::from("-vf"),
        OsString::from("scale=1:1"),
        OsString::from("-frames:v"),
        OsString::from("1"),
        OsString::from("-f"),
        OsString::from("rawvideo"),
        OsString::from("-pix_fmt"),
        OsString::from("rgb24"),
        OsString::from("-"),
    ]
}

/// Writes one picture at one width.
pub async fn resize(tool: &Path, source: &Path, destination: &Path, width: u32) -> Result<()> {
    let output = TokioCommand::new(tool)
        .args(resize_arguments(source, destination, width))
        .stdin(Stdio::null())
        .output()
        .await?;

    if !output.status.success() {
        return Err(failure(&output));
    }
    Ok(())
}

/// Reads the average colour of a picture, as it is written in a stylesheet.
pub async fn average_colour(tool: &Path, source: &Path) -> Result<String> {
    let output = TokioCommand::new(tool)
        .args(average_colour_arguments(source))
        .stdin(Stdio::null())
        .output()
        .await?;

    if !output.status.success() {
        return Err(failure(&output));
    }
    to_hex(&output.stdout).ok_or_else(|| FfmpegError::Failed {
        tool: "ffmpeg",
        status: "0".to_string(),
        output: "no pixel came back to read a colour from".to_string(),
    })
}

fn failure(output: &std::process::Output) -> FfmpegError {
    FfmpegError::Failed {
        tool: "ffmpeg",
        status: output.status.to_string(),
        output: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    }
}

/// Turns three bytes into the form a stylesheet takes.
fn to_hex(pixel: &[u8]) -> Option<String> {
    let [red, green, blue] = pixel.get(..3)? else {
        return None;
    };
    Some(format!("#{red:02x}{green:02x}{blue:02x}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rendered(arguments: &[OsString]) -> String {
        arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn a_resize_keeps_the_shape_of_the_picture() {
        let arguments = resize_arguments(
            &PathBuf::from("/cache/source.jpg"),
            &PathBuf::from("/cache/poster-400.webp"),
            400,
        );
        let line = rendered(&arguments);

        assert!(line.contains("scale=400:-1"), "{line}");
        assert!(
            line.contains("libwebp"),
            "the interface is served pictures a browser reads without thinking: {line}"
        );
        assert!(
            line.contains("-y"),
            "a picture generated again replaces itself"
        );
    }

    #[test]
    fn reading_a_colour_asks_for_one_pixel_and_nothing_else() {
        let line = rendered(&average_colour_arguments(&PathBuf::from("/cache/p.jpg")));
        assert!(line.contains("scale=1:1"), "{line}");
        assert!(line.contains("rgb24"), "{line}");
        assert!(
            line.ends_with('-'),
            "the pixel comes back on the output rather than through a file: {line}"
        );
    }

    #[test]
    fn a_pixel_becomes_the_form_a_stylesheet_takes() {
        assert_eq!(to_hex(&[0xc8, 0x1e, 0x1e]).as_deref(), Some("#c81e1e"));
        assert_eq!(to_hex(&[0, 0, 0]).as_deref(), Some("#000000"));
        assert_eq!(to_hex(&[255, 255, 255]).as_deref(), Some("#ffffff"));
    }

    #[test]
    fn half_a_pixel_is_not_a_colour() {
        assert_eq!(to_hex(&[]), None);
        assert_eq!(to_hex(&[1, 2]), None);
    }

    #[test]
    fn the_widths_offered_go_from_a_card_to_a_page_header() {
        assert!(POSTER_WIDTHS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(BACKDROP_WIDTHS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(
            BACKDROP_WIDTHS[0] > POSTER_WIDTHS[0],
            "a backdrop is shown wide and a poster is not"
        );
        assert!(PHOTO_WIDTHS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(
            PHOTO_WIDTHS[0] < POSTER_WIDTHS[0],
            "a face in a round frame is the smallest picture prepared"
        );
    }
}
