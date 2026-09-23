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

use melyxar_core::orientation::Orientation;
use melyxar_core::time::Millis;
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
/// Backdrops are shown wide, so they start where posters end and go as far as
/// the screens they are drawn on: a banner covers the whole top of one, so
/// its width is the screen's own. The tile of a way in, a laptop, most
/// monitors, a wide desk monitor, and a four thousand line screen.
///
/// Nothing here is ever written larger than the picture that arrived, so a
/// film the provider holds small costs the first of these and no more.
pub const BACKDROP_WIDTHS: [u32; 5] = [640, 1280, 1920, 2560, 3840];
/// A face is shown in a small round frame, and a screen with fine pixels wants
/// twice what it measures. Two widths cover both and no more.
pub const PHOTO_WIDTHS: [u32; 2] = [96, 192];
/// A title image is drawn in a box a few hundred points across, beside the way
/// back out of a film. Like a face, it wants what it measures and twice that,
/// and nothing beyond: it is never shown large.
pub const LOGO_WIDTHS: [u32; 2] = [340, 680];

/// Builds the conversion of one picture to every width at once.
///
/// One run rather than one per width, because reading the picture and starting
/// the tool cost more than the scaling does: a film brings twenty pictures and
/// a library brings a hundred films, so the difference is minutes.
///
/// The height follows the width so nothing is ever stretched, and an odd
/// number of pixels is allowed here: unlike video, a picture has no encoder
/// demanding even sides.
pub fn resize_arguments(source: &Path, destinations: &[(u32, &Path)]) -> Vec<OsString> {
    let mut arguments = vec![
        OsString::from("-hide_banner"),
        OsString::from("-loglevel"),
        OsString::from("error"),
        OsString::from("-y"),
        OsString::from("-i"),
        source.as_os_str().to_os_string(),
    ];

    // The picture is read once and handed to every scaling, which is what lets
    // the outputs share the one run.
    let mut graph = format!("[0:v]split={}", destinations.len());
    for index in 0..destinations.len() {
        graph.push_str(&format!("[in{index}]"));
    }
    for (index, (width, _)) in destinations.iter().enumerate() {
        graph.push_str(&format!(
            ";[in{index}]scale={width}:-1:flags=lanczos[out{index}]"
        ));
    }
    arguments.push(OsString::from("-filter_complex"));
    arguments.push(OsString::from(graph));

    for (index, (_, destination)) in destinations.iter().enumerate() {
        arguments.push(OsString::from("-map"));
        arguments.push(OsString::from(format!("[out{index}]")));
        arguments.push(OsString::from("-frames:v"));
        arguments.push(OsString::from("1"));
        arguments.push(OsString::from("-c:v"));
        arguments.push(OsString::from("libwebp"));
        arguments.push(OsString::from("-quality"));
        arguments.push(OsString::from(QUALITY.to_string()));
        arguments.push(destination.as_os_str().to_os_string());
    }
    arguments
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

/// Writes one picture at every width the interface serves.
pub async fn resize(tool: &Path, source: &Path, destinations: &[(u32, &Path)]) -> Result<()> {
    if destinations.is_empty() {
        return Ok(());
    }
    let output = TokioCommand::new(tool)
        .args(resize_arguments(source, destinations))
        .stdin(Stdio::null())
        .output()
        .await?;

    if !output.status.success() {
        return Err(FfmpegError::from_output("ffmpeg", &output));
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
        return Err(FfmpegError::from_output("ffmpeg", &output));
    }
    to_hex(&output.stdout).ok_or_else(|| FfmpegError::Failed {
        tool: "ffmpeg",
        status: "0".to_string(),
        output: "no pixel came back to read a colour from".to_string(),
    })
}

/// How many points across a profile picture is written: twice the largest
/// round it is shown in, for a screen with fine pixels.
pub const AVATAR_SIDE: u32 = 256;

/// Builds the making of a profile picture out of whatever image somebody
/// sent: turned the way its camera said, cut square around its middle, and
/// brought down to one size. Never enlarged, since that invents nothing.
pub fn avatar_arguments(
    source: &Path,
    orientation: Orientation,
    destination: &Path,
) -> Vec<OsString> {
    let square =
        format!("crop='min(iw,ih)':'min(iw,ih)',scale='min(iw,{AVATAR_SIDE})':-1:flags=lanczos");
    let filter = match turn_of(orientation) {
        Some(turn) => format!("{turn},{square}"),
        None => square,
    };
    vec![
        OsString::from("-hide_banner"),
        OsString::from("-loglevel"),
        OsString::from("error"),
        OsString::from("-y"),
        OsString::from("-noautorotate"),
        OsString::from("-i"),
        source.as_os_str().to_os_string(),
        OsString::from("-vf"),
        OsString::from(filter),
        OsString::from("-frames:v"),
        OsString::from("1"),
        OsString::from("-c:v"),
        OsString::from("libwebp"),
        OsString::from("-quality"),
        OsString::from(QUALITY.to_string()),
        destination.as_os_str().to_os_string(),
    ]
}

/// Makes a profile picture out of an image somebody sent.
pub async fn avatar(
    tool: &Path,
    source: &Path,
    orientation: Orientation,
    destination: &Path,
) -> Result<()> {
    let output = TokioCommand::new(tool)
        .args(avatar_arguments(source, orientation, destination))
        .stdin(Stdio::null())
        .output()
        .await?;
    if !output.status.success() {
        return Err(FfmpegError::from_output("ffmpeg", &output));
    }
    Ok(())
}

/// Builds the taking of one picture, the right way up, out of a file somebody
/// filmed or photographed themselves.
///
/// Out of a video, the picture at that moment, which the tool turns on its
/// own when the video says it was filmed on its side. Out of a photo, the
/// photo, turned the way its camera said: the tool is told not to turn it
/// itself, so that no version of it that has learnt to read the camera's note
/// turns it twice.
///
/// Written whole and without loss, since it is only what the sizes a card is
/// shown at are made from.
pub fn upright_picture_arguments(
    source: &Path,
    at: Option<Millis>,
    orientation: Orientation,
    destination: &Path,
) -> Vec<OsString> {
    let mut arguments = vec![
        OsString::from("-hide_banner"),
        OsString::from("-loglevel"),
        OsString::from("error"),
        OsString::from("-y"),
    ];
    match at {
        Some(moment) => {
            arguments.push(OsString::from("-ss"));
            arguments.push(OsString::from(format!("{:.3}", moment.as_seconds_f64())));
        }
        None => arguments.push(OsString::from("-noautorotate")),
    }
    arguments.push(OsString::from("-i"));
    arguments.push(source.as_os_str().to_os_string());
    if let Some(turn) = turn_of(orientation) {
        arguments.push(OsString::from("-vf"));
        arguments.push(OsString::from(turn));
    }
    arguments.extend([
        OsString::from("-frames:v"),
        OsString::from("1"),
        OsString::from("-c:v"),
        OsString::from("png"),
        destination.as_os_str().to_os_string(),
    ]);
    arguments
}

/// The filter that turns a photo the way its camera said.
fn turn_of(orientation: Orientation) -> Option<&'static str> {
    match orientation {
        Orientation::AsStored => None,
        Orientation::Mirrored => Some("hflip"),
        Orientation::UpsideDown => Some("hflip,vflip"),
        Orientation::MirroredUpsideDown => Some("vflip"),
        Orientation::Transposed => Some("transpose=cclock_flip"),
        Orientation::TurnedRight => Some("transpose=clock"),
        Orientation::Transversed => Some("transpose=clock_flip"),
        Orientation::TurnedLeft => Some("transpose=cclock"),
    }
}

/// Takes one picture, the right way up, out of a video or a photo.
pub async fn upright_picture(
    tool: &Path,
    source: &Path,
    at: Option<Millis>,
    orientation: Orientation,
    destination: &Path,
) -> Result<()> {
    let output = TokioCommand::new(tool)
        .args(upright_picture_arguments(
            source,
            at,
            orientation,
            destination,
        ))
        .stdin(Stdio::null())
        .output()
        .await?;
    if !output.status.success() {
        return Err(FfmpegError::from_output("ffmpeg", &output));
    }
    Ok(())
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
        let small = PathBuf::from("/cache/poster-400.webp");
        let arguments = resize_arguments(&PathBuf::from("/cache/source.jpg"), &[(400, &small)]);
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
    fn every_width_is_written_by_one_run_that_reads_the_picture_once() {
        let small = PathBuf::from("/cache/poster-200.webp");
        let large = PathBuf::from("/cache/poster-800.webp");
        let line = rendered(&resize_arguments(
            &PathBuf::from("/cache/source.jpg"),
            &[(200, &small), (800, &large)],
        ));

        assert_eq!(line.matches(" -i ").count(), 1, "read once: {line}");
        assert!(line.contains("split=2"), "{line}");
        assert!(line.contains("[in0]scale=200:-1"), "{line}");
        assert!(line.contains("[in1]scale=800:-1"), "{line}");
        assert!(
            line.contains(
                "-map [out0] -frames:v 1 -c:v libwebp -quality 80 /cache/poster-200.webp"
            ),
            "each width is an output of its own: {line}"
        );
        assert!(line.ends_with("/cache/poster-800.webp"), "{line}");
    }

    /// The command above is only worth anything if the tool accepts it, and
    /// nothing but the tool can say so.
    #[tokio::test]
    async fn the_tool_really_writes_every_width_in_one_run() {
        let folder = tempfile::tempdir().expect("temporary directory");
        let source = folder.path().join("source.jpg");
        let made = TokioCommand::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=#3366aa:s=1000x1500",
                "-frames:v",
                "1",
            ])
            .arg(&source)
            .output()
            .await;
        if !made.is_ok_and(|output| output.status.success()) {
            eprintln!("no media tool here, the conversion was not exercised");
            return;
        }

        let small = folder.path().join("poster-200.webp");
        let large = folder.path().join("poster-800.webp");
        resize(
            Path::new("ffmpeg"),
            &source,
            &[(200, &small), (800, &large)],
        )
        .await
        .expect("the tool accepted the command");

        for (path, least) in [(&small, 100), (&large, 1_000)] {
            let written = std::fs::metadata(path).expect("a file was written");
            assert!(
                written.len() > least,
                "{path:?} came out at {} bytes",
                written.len()
            );
        }
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
    fn a_photo_is_turned_by_what_its_camera_said_and_never_twice() {
        let photo = rendered(&upright_picture_arguments(
            Path::new("/media/beach.jpg"),
            None,
            Orientation::TurnedRight,
            Path::new("/cache/upright.png"),
        ));
        assert!(
            photo.contains("-noautorotate -i /media/beach.jpg"),
            "{photo}"
        );
        assert!(photo.contains("-vf transpose=clock"), "{photo}");

        let untouched = rendered(&upright_picture_arguments(
            Path::new("/media/beach.jpg"),
            None,
            Orientation::AsStored,
            Path::new("/cache/upright.png"),
        ));
        assert!(!untouched.contains("-vf"), "{untouched}");
    }

    #[test]
    fn a_video_gives_the_picture_at_the_moment_asked_turned_by_the_tool() {
        let video = rendered(&upright_picture_arguments(
            Path::new("/media/birthday.mp4"),
            Some(Millis::new(12_500)),
            Orientation::AsStored,
            Path::new("/cache/upright.png"),
        ));
        assert!(
            video.contains("-ss 12.500 -i /media/birthday.mp4"),
            "{video}"
        );
        assert!(
            !video.contains("-noautorotate"),
            "a video filmed on its side is turned by the tool itself: {video}"
        );
        assert!(video.ends_with("-frames:v 1 -c:v png /cache/upright.png"));
    }

    #[test]
    fn a_profile_picture_is_turned_then_cut_square_then_brought_down() {
        let turned = rendered(&avatar_arguments(
            Path::new("/data/sent.source"),
            Orientation::TurnedLeft,
            Path::new("/data/avatar.webp"),
        ));
        assert!(
            turned.contains("-noautorotate -i /data/sent.source"),
            "{turned}"
        );
        assert!(
            turned.contains(
                "-vf transpose=cclock,crop='min(iw,ih)':'min(iw,ih)',scale='min(iw,256)':-1"
            ),
            "turned before it is cut, or the square is taken from the wrong side: {turned}"
        );
        assert!(turned.ends_with("-c:v libwebp -quality 80 /data/avatar.webp"));

        let upright = rendered(&avatar_arguments(
            Path::new("/data/sent.source"),
            Orientation::AsStored,
            Path::new("/data/avatar.webp"),
        ));
        assert!(upright.contains("-vf crop="), "{upright}");
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
        assert!(LOGO_WIDTHS.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            LOGO_WIDTHS[1],
            LOGO_WIDTHS[0] * 2,
            "the second width exists for a screen with fine pixels, \
             so it is exactly twice the first"
        );
    }

    /// A title image is the only picture here carrying transparency, and it is
    /// drawn over a film: were it flattened, the wordmark would arrive in a
    /// box of its own colour. Nothing in the command asks for transparency to
    /// be kept, so this is what says it survives all the same.
    #[tokio::test]
    async fn a_picture_that_sees_through_itself_still_does_afterwards() {
        let folder = tempfile::tempdir().expect("temporary directory");
        let source = folder.path().join("source.png");
        let made = TokioCommand::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=#ffffff@0.0:s=400x200,format=rgba",
                "-frames:v",
                "1",
            ])
            .arg(&source)
            .output()
            .await;
        if !made.is_ok_and(|output| output.status.success()) {
            eprintln!("no media tool here, the conversion was not exercised");
            return;
        }

        let written = folder.path().join("logo-340.webp");
        resize(Path::new("ffmpeg"), &source, &[(340, &written)])
            .await
            .expect("the tool accepted the command");

        let read_back = TokioCommand::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-i"])
            .arg(&written)
            .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .output()
            .await
            .expect("the picture was read back");
        assert_eq!(
            read_back.stdout.get(3),
            Some(&0),
            "the first pixel was fully see-through and must still be"
        );
    }
}
