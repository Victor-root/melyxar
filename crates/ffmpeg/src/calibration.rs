//! The reference film a browser is measured against.
//!
//! Calibrating a client means finding out what it really decodes, rather than
//! what it says about a codec in the abstract. That has to be asked against
//! the same picture every time, on every installation, without depending on
//! anything a viewer happens to have in a library, a file fetched from
//! somewhere, or a licence to think about. So the picture is made on the
//! spot, exactly the way a card's own capability is proved in [`crate::hardware`]:
//! a generated source, encoded once, and kept.
//!
//! What is generated is not a test card. A flat picture compresses to almost
//! nothing and tells a decoder almost nothing about itself; a real film keeps
//! changing in ways that force real work every frame. A fractal zoom does the
//! same without needing a single frame of anyone's footage.

use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;

use melyxar_core::time::Millis;
use tokio::process::Command as TokioCommand;

use crate::{FfmpegError, Result, ToolPaths};

/// Shape of the reference film. The size a calibration is actually asked
/// about, since nothing taller than this is ever offered to a client to
/// decode.
pub const REFERENCE_WIDTH: u32 = 3840;
pub const REFERENCE_HEIGHT: u32 = 2160;
pub const REFERENCE_FRAME_RATE: u32 = 24;

/// How long the reference film runs.
///
/// Long enough that a decoder settles into its real behaviour rather than the
/// first moment of a stream, short enough that testing every codec at every
/// size a calibration cares about stays inside the time a viewer was told it
/// would take.
pub const REFERENCE_DURATION: Millis = Millis::new(12_000);

/// The encoder the reference film is made with.
///
/// The one every build carries, so making it never depends on the same
/// hardware path a calibration exists to find the truth about.
const MASTER_ENCODER: &str = "libx264";

/// Builds the command that generates the reference film.
///
/// The length is a parameter of its own, apart from [`REFERENCE_DURATION`],
/// so that what is worth proving about the command, generating a real file
/// with real content in it, can be proved on a length of a fraction of a
/// second instead of the full length a calibration actually uses.
fn arguments(into: &Path, duration: Millis) -> Vec<OsString> {
    [
        "-hide_banner",
        "-nostdin",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        // maxiter held low on purpose: the filter's own default computes
        // thousands of iterations a pixel, which turns making this file once
        // into a multi-minute wait for nothing this measures. A hundred is
        // still a picture that changes in full every frame.
        &format!(
            "mandelbrot=size={REFERENCE_WIDTH}x{REFERENCE_HEIGHT}:rate={REFERENCE_FRAME_RATE}:maxiter=100"
        ),
        "-t",
        &duration.as_seconds_f64().to_string(),
        // No soundtrack: this measures a picture, and the pipeline it travels
        // through already handles a film that carries none.
        "-an",
        "-c:v",
        MASTER_ENCODER,
        // Speed over compression efficiency: this file is made once, cached,
        // and never sent anywhere. Quality is held by the rate factor below,
        // which a preset does not change.
        "-preset",
        "ultrafast",
        // Close to lossless: what is measured later is the browser's decode
        // of the codec a calibration rebuilds this into, and that answer must
        // not be shaped by how hard this master was already compressed.
        "-crf",
        "12",
        "-pix_fmt",
        "yuv420p",
        into.as_os_str().to_string_lossy().as_ref(),
    ]
    .map(OsString::from)
    .to_vec()
}

/// Runs the command above and turns a failing exit status into an error.
async fn run(tools: &ToolPaths, into: &Path, duration: Millis) -> Result<()> {
    let output = TokioCommand::new(&tools.ffmpeg)
        .args(arguments(into, duration))
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

/// Makes the reference film, or says why it could not.
///
/// Never asked twice for the same file: the caller keeps it once made, the
/// same way every other generated picture in this cache is kept.
pub async fn make_reference_film(tools: &ToolPaths, into: &Path) -> Result<()> {
    run(tools, into, REFERENCE_DURATION).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_film_is_made_from_nothing_a_library_holds() {
        let built: Vec<String> = arguments(Path::new("/tmp/reference.mkv"), REFERENCE_DURATION)
            .into_iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect();
        assert!(built.iter().any(|value| value == "lavfi"));
        assert!(built.iter().any(|value| value.starts_with("mandelbrot=")));
        assert!(built.iter().any(|value| value == "-an"));
        assert!(built.iter().any(|value| value == "12"));
        assert!(built.last().unwrap().ends_with("reference.mkv"));
    }

    #[tokio::test]
    async fn the_reference_film_is_really_made_on_this_machine() {
        // A fraction of a second is enough to prove the command really runs
        // and really writes a readable film: what is worth proving here is
        // the command, not the twelve seconds a calibration actually asks
        // for, which the argument test above already pins.
        let tools = crate::ToolPaths::discover(None, None).expect("the tools are installed here");
        let directory = tempfile::tempdir().expect("temporary directory");
        let into = directory.path().join("reference.mkv");

        run(&tools, &into, Millis::new(500))
            .await
            .expect("the reference film is made");
        let metadata = std::fs::metadata(&into).expect("the reference film was written");
        assert!(metadata.len() > 0, "a real film, not an empty file");

        let probed = crate::probe::probe(&tools.ffprobe, &into)
            .await
            .expect("the reference film can be read back");
        assert!(probed.describes_something());
    }
}
