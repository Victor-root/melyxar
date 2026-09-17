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
//!
//! A fractal alone is not enough, and proved so on real hardware: its shapes
//! are large and smooth from one pixel to the next, so an encoder reaches the
//! bitrate a real film would need without spending it, and a decoder with no
//! real hardware support for a codec can keep up with the result even though
//! it cannot keep up with an actual film in that codec. Grain is what a real
//! film has that a fractal does not: real, uncorrelated detail in every
//! pixel, of every frame, that cannot be predicted from its neighbours or
//! from the frame before it. It is layered on top of the fractal so the
//! reference film is forced to spend close to the same bitrate a real film
//! would, on the same kind of detail a decoder cannot shortcut.

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

/// Which recipe the reference film on disk was made by.
///
/// Bumped whenever the arguments below change in a way that changes the film
/// itself, the fractal, the grain, the size or the length, so a copy made by
/// an older recipe is never mistaken for this one and kept forever. The
/// caller that keeps the file on disk carries this in its own name.
pub const REFERENCE_RECIPE_VERSION: u32 = 2;

/// The encoder the reference film is made with.
///
/// The one every build carries, so making it never depends on the same
/// hardware path a calibration exists to find the truth about.
const MASTER_ENCODER: &str = "libx264";

/// How strong the grain layered onto the fractal is, on the `noise` filter's
/// own 0-100 scale.
///
/// Chosen high enough that the film cannot be told apart from real, grainy
/// footage by how many real bits it costs a codec to keep, and low enough
/// that the master this is layered onto, kept close to lossless so the grain
/// survives it, stays a reasonable size to generate once and cache.
const GRAIN_STRENGTH: u32 = 20;

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
        // The fractal supplies a picture that keeps changing; the noise
        // chained after it supplies the per-pixel, per-frame detail a real
        // film's grain would, which is what actually costs a codec its bits
        // and a decoder its work. `t` keeps the grain pattern itself
        // changing every frame, so it is never something a decoder could
        // predict from the frame before.
        &format!(
            "mandelbrot=size={REFERENCE_WIDTH}x{REFERENCE_HEIGHT}:rate={REFERENCE_FRAME_RATE}:maxiter=100,noise=alls={GRAIN_STRENGTH}:allf=t+u"
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
        assert!(built.iter().any(|value| value.contains("noise=")));
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
