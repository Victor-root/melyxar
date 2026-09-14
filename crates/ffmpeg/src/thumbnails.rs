//! The little pictures shown while somebody drags along the playback bar.
//!
//! Not the pictures of a film's artwork, which are resized next door: these
//! are read out of the film itself, hundreds at a time.
//!
//! A film of two hours holds seven hundred of them at one every ten seconds.
//! Written one to a file, dragging along the bar would mean seven hundred
//! requests, dozens of them a second while the finger is moving. So they are
//! gathered a hundred at a time into sheets: one request covers a thousand
//! seconds of film, and the next sheet is fetched long before it is reached.
//! This is what Jellyfin and Emby do, and it is why.
//!
//! One reading of the film produces every sheet. The reading is the whole
//! cost, exactly as it is for subtitles, and only the pictures that stand on
//! their own are decoded: the rest are thrown away before they are rebuilt,
//! which is measured here at four times faster and changes nothing anybody
//! sees at this size.

use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;

use melyxar_core::thumbnails::{Layout, Thumbnails};
use tokio::process::Command as TokioCommand;

use crate::{FfmpegError, Result, ToolPaths};

/// Quality of the sheets, on the scale the tool uses: one is best, thirty one
/// is worst. Four is where a thumbnail of this size stops improving.
const QUALITY: u8 = 4;

/// The name the sheets are written under, which the tool fills in itself.
///
/// Numbered from zero so that the sheet holding a thumbnail is its number
/// divided by what a sheet holds, with nothing to add or take away.
const SHEET_NAMES: &str = "%04d.jpg";

/// Builds the reading of one film for its thumbnails.
///
/// One reading, two outputs. The first gathers the pictures into sheets. The
/// second is a single pixel per thumbnail, thrown at the output of the tool
/// and never kept: counting those bytes is how the number of thumbnails is
/// known to be the number that came out, rather than the number a running time
/// promised. A container's running time is wrong often enough that nothing may
/// be built on it, and a film whose last stretch is not a whole one ends a
/// thumbnail short of what it suggests.
pub fn arguments(
    source: &Path,
    into: &Path,
    layout: Layout,
    tone_map: bool,
    standing_pictures_only: bool,
) -> Vec<OsString> {
    let mut arguments = vec![
        OsString::from("-hide_banner"),
        OsString::from("-loglevel"),
        OsString::from("error"),
        OsString::from("-y"),
    ];

    if standing_pictures_only {
        // Said before the input, because it is the reading of the input it
        // changes: the tool throws away every picture that leans on another
        // one before rebuilding it.
        arguments.push(OsString::from("-skip_frame"));
        arguments.push(OsString::from("nokey"));
    }
    arguments.push(OsString::from("-i"));
    arguments.push(source.as_os_str().to_os_string());
    // Neither the sound nor the words have anything to do with a picture.
    arguments.push(OsString::from("-an"));
    arguments.push(OsString::from("-sn"));

    // Thinned out first, then made smaller, then converted: every stage costs
    // what the stage before it left, and the first one leaves one picture in a
    // few hundred.
    // Which picture of a ten second stretch is the one shown, and it is not a
    // detail: rounded the usual way the filter hands over the last picture of
    // the stretch, and measured on a film with the time written into it, the
    // thumbnail for the very start of the film showed four point nine seconds.
    // Rounded up it is the picture at the mark itself.
    //
    // Counted from nought rather than from the first picture of the film, and
    // that is the other half. Without it the grid is anchored on whatever the
    // decoder hands over first, which is not the same thing in every codec:
    // measured on the same film in two forms, one came out right and the other
    // was a whole stretch out. Anchored on nought, both are the picture at the
    // mark, within one frame of it.
    let mut chain = format!(
        "[0:v]fps=1000/{}:round=up:start_time=0,scale=-2:{}",
        layout.every.get().max(1),
        layout.height
    );
    if tone_map {
        chain.push(',');
        chain.push_str(crate::command::TONE_MAP_FILTER);
    }
    chain.push_str(&format!(
        ",split=2[grid][tally];[grid]tile={}x{}[sheets];[tally]scale=1:1[one]",
        layout.columns, layout.rows
    ));
    arguments.push(OsString::from("-filter_complex"));
    arguments.push(OsString::from(chain));

    arguments.push(OsString::from("-map"));
    arguments.push(OsString::from("[sheets]"));
    arguments.push(OsString::from("-qscale:v"));
    arguments.push(OsString::from(QUALITY.to_string()));
    arguments.push(OsString::from("-f"));
    arguments.push(OsString::from("image2"));
    arguments.push(OsString::from("-start_number"));
    arguments.push(OsString::from("0"));
    arguments.push(into.join(SHEET_NAMES).as_os_str().to_os_string());

    arguments.push(OsString::from("-map"));
    arguments.push(OsString::from("[one]"));
    arguments.push(OsString::from("-f"));
    arguments.push(OsString::from("rawvideo"));
    arguments.push(OsString::from("-pix_fmt"));
    arguments.push(OsString::from("gray"));
    arguments.push(OsString::from("-"));
    arguments
}

/// Where one sheet of a film is written.
pub fn sheet_at(into: &Path, number: u32) -> std::path::PathBuf {
    into.join(format!("{number:04}.jpg"))
}

/// Reads one film through and writes every sheet of thumbnails it gives.
///
/// Answers what really came out, measured rather than worked out: how many
/// thumbnails, how many sheets, and the size of one picture on a sheet.
///
/// The folder is written into as it stands. Emptying it first is the caller's
/// to do: a film read again is a film whose old sheets describe nothing.
///
/// A file that holds no picture at all comes back with nothing counted rather
/// than as a fault: it is an answer about that file, and one worth writing
/// down so the file is never read through again for the same nothing.
pub async fn make(
    tools: &ToolPaths,
    source: &Path,
    into: &Path,
    layout: Layout,
    tone_map: bool,
    standing_pictures_only: bool,
) -> Result<Thumbnails> {
    let output = TokioCommand::new(&tools.ffmpeg)
        .args(arguments(
            source,
            into,
            layout,
            tone_map,
            standing_pictures_only,
        ))
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

    // One byte was asked for per thumbnail, so this is their number.
    let counted = output.stdout.len() as u32;
    if counted == 0 {
        // A reading that went well and gave nothing is an answer, not a
        // fault: there are files in a film folder that hold no picture. Said
        // as an answer so it is written down and the file is never read
        // through again for the same nothing.
        return Ok(Thumbnails {
            every: layout.every,
            width: 0,
            height: 0,
            columns: layout.columns,
            rows: layout.rows,
            counted: 0,
            sheets: 0,
        });
    }
    let sheets = counted.div_ceil(layout.per_sheet().max(1));

    // Measured on the sheet rather than worked out from the film: a film whose
    // pixels are not square comes out a different shape than its numbers say,
    // and the page places every thumbnail by these two numbers.
    let first = sheet_at(into, 0);
    let (sheet_width, sheet_height) = size_of(&tools.ffprobe, &first).await?;
    Ok(Thumbnails {
        every: layout.every,
        width: sheet_width / layout.columns.max(1),
        height: sheet_height / layout.rows.max(1),
        columns: layout.columns,
        rows: layout.rows,
        counted,
        sheets,
    })
}

/// How large a picture is, asked of the analyser.
async fn size_of(analyser: &Path, picture: &Path) -> Result<(u32, u32)> {
    let report = crate::probe::probe(analyser, picture).await?;
    report
        .streams
        .iter()
        .find_map(|stream| Some((stream.width? as u32, stream.height? as u32)))
        .ok_or_else(|| {
            FfmpegError::MalformedReport("a sheet of thumbnails has no size".to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::time::Millis;
    use std::path::PathBuf;

    fn ten_seconds() -> Layout {
        Layout {
            every: Millis::new(10_000),
            height: 180,
            columns: 10,
            rows: 10,
        }
    }

    fn rendered(arguments: &[OsString]) -> String {
        arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn one_reading_writes_every_sheet_of_a_film() {
        let line = rendered(&arguments(
            &PathBuf::from("/films/Quiet.Harbour.2019.mkv"),
            &PathBuf::from("/cache/trickplay/one"),
            ten_seconds(),
            false,
            true,
        ));

        assert_eq!(line.matches(" -i ").count(), 1, "read once: {line}");
        assert!(
            line.contains("fps=1000/10000:round=up:start_time=0"),
            "{line}"
        );
        assert!(line.contains("tile=10x10"), "{line}");
        assert!(
            line.contains("-start_number 0"),
            "the sheet holding a thumbnail is its number divided by what a \
             sheet holds, so the first sheet is sheet nought: {line}"
        );
        assert!(line.contains("/cache/trickplay/one/%04d.jpg"), "{line}");
    }

    #[test]
    fn only_the_pictures_that_stand_on_their_own_are_rebuilt() {
        // Four times faster, measured, and the difference is invisible at this
        // size. Said before the input, because it is the reading it changes.
        let line = rendered(&arguments(
            &PathBuf::from("/films/Quiet.Harbour.2019.mkv"),
            &PathBuf::from("/cache/trickplay/one"),
            ten_seconds(),
            false,
            true,
        ));
        let skip = line.find("-skip_frame nokey").expect("asked for");
        assert!(skip < line.find(" -i ").expect("an input"), "{line}");

        let every = rendered(&arguments(
            &PathBuf::from("/films/Quiet.Harbour.2019.mkv"),
            &PathBuf::from("/cache/trickplay/one"),
            ten_seconds(),
            false,
            false,
        ));
        assert!(!every.contains("-skip_frame"), "{every}");
    }

    #[test]
    fn the_thumbnails_are_counted_rather_than_worked_out() {
        let line = rendered(&arguments(
            &PathBuf::from("/films/Quiet.Harbour.2019.mkv"),
            &PathBuf::from("/cache/trickplay/one"),
            ten_seconds(),
            false,
            true,
        ));
        assert!(line.contains("[tally]scale=1:1[one]"), "{line}");
        assert!(
            line.ends_with("-map [one] -f rawvideo -pix_fmt gray -"),
            "one byte a thumbnail, on the output of the tool: {line}"
        );
    }

    #[test]
    fn the_picture_shown_is_the_one_at_the_mark_and_not_the_one_before_the_next() {
        // Two halves of one answer, both measured on a film with the time
        // written into it. Rounded the usual way, the filter hands over the
        // last picture of each stretch: the thumbnail for the very start of
        // the film showed four point nine seconds. Anchored on the first
        // picture the decoder happens to hand over rather than on nought, the
        // same film in another form was a whole stretch out.
        let line = rendered(&arguments(
            &PathBuf::from("/films/Quiet.Harbour.2019.mkv"),
            &PathBuf::from("/cache/thumbnails/one"),
            ten_seconds(),
            false,
            true,
        ));
        assert!(line.contains(":round=up"), "{line}");
        assert!(line.contains(":start_time=0"), "{line}");
    }

    #[test]
    fn a_wide_gamut_film_is_converted_before_it_is_gathered() {
        // An unconverted frame is the washed out thumbnail seen elsewhere.
        let line = rendered(&arguments(
            &PathBuf::from("/films/Quiet.Harbour.2019.mkv"),
            &PathBuf::from("/cache/trickplay/one"),
            ten_seconds(),
            true,
            true,
        ));
        let converted = line.find("tonemap=").expect("converted");
        let smaller = line.find("scale=-2:180").expect("made smaller");
        let gathered = line.find("tile=").expect("gathered");
        assert!(
            smaller < converted && converted < gathered,
            "smaller first, because converting colours costs per pixel: {line}"
        );
    }

    #[test]
    fn a_sheet_is_named_by_its_number_alone() {
        let folder = PathBuf::from("/cache/trickplay/one");
        assert_eq!(sheet_at(&folder, 0), folder.join("0000.jpg"));
        assert_eq!(sheet_at(&folder, 12), folder.join("0012.jpg"));
    }

    /// The command is worth nothing unless the tool accepts it, and nothing
    /// but the tool can say so.
    #[tokio::test]
    async fn a_real_film_gives_up_its_thumbnails_in_one_reading() {
        let Ok(tools) = crate::ToolPaths::discover(None, None) else {
            eprintln!("no media tool here, the reading was not exercised");
            return;
        };
        let directory = tempfile::tempdir().expect("temporary directory");
        let film = directory.path().join("Quiet.Harbour.2019.mkv");
        // Twenty five seconds, so three thumbnails at one every ten.
        let made = TokioCommand::new(&tools.ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=12:duration=25",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-g",
                "24",
            ])
            .arg(&film)
            .output()
            .await
            .expect("the tool runs");
        assert!(made.status.success(), "a film to read");

        let into = directory.path().join("thumbnails");
        std::fs::create_dir_all(&into).expect("a folder to write in");
        let made = make(
            &tools,
            &film,
            &into,
            Layout {
                every: Millis::new(10_000),
                height: 90,
                columns: 2,
                rows: 2,
            },
            false,
            false,
        )
        .await
        .expect("the tool accepted the command");

        assert_eq!(made.counted, 3, "nought, ten and twenty seconds");
        assert_eq!(made.sheets, 1);
        assert_eq!(made.height, 90);
        assert_eq!(made.width, 160, "the shape of the film is kept");
        assert!(
            sheet_at(&into, 0).exists(),
            "the sheets are numbered from nought"
        );
    }
}
