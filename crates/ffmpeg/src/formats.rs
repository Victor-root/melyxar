//! Which container formats the tool is allowed to open a file as.
//!
//! The tool does not trust a file's name for what it is: it reads the first
//! bytes and picks a reader to match. A handful of those readers are not
//! readers of one file at all but of a list that names others to open, meant
//! for streaming over a network: given one, the tool goes and opens whatever
//! it names. A file brought in from anywhere, disguised as an ordinary video,
//! could so make the tool read a file from another library, or reach out
//! across the network.
//!
//! So every reading of a file the server did not make itself is told the set
//! of formats it may be: every format this build of the tool knows, less
//! those few list formats. A file that is none of them is refused before a
//! byte of it is decoded, and nothing a real film, song, picture or subtitle
//! could be is ever turned away, whatever the build.

use std::ffi::OsString;

/// The readers that open other inputs rather than hold media of their own.
///
/// Named as the tool names them, one word each. Left out of what a file may
/// be, so a file is never read as one.
const OPENS_OTHER_INPUTS: [&str; 6] =
    ["dash", "hls", "concat", "imf", "sdp", "webm_dash_manifest"];

/// The formats a file may be, from the list the tool gives of every reader it
/// has (`ffmpeg -demuxers`), less the ones that open other inputs.
///
/// Read from the tool itself rather than written down here, so a build with a
/// reader this code never heard of still opens the files it is for, and a file
/// is never refused for being a perfectly ordinary thing this code did not
/// know to name.
pub(crate) fn allowed_from(demuxers: &str) -> String {
    let names = demuxers
        .lines()
        // A real entry is " D  name"; the lines about the listing itself
        // (" D. = Demuxing supported" and the like) do not have the space
        // after the D that a reader's flag column does.
        .filter_map(|line| line.strip_prefix(" D "))
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| !OPENS_OTHER_INPUTS.contains(name));
    let mut allowed = String::new();
    for name in names {
        if !allowed.is_empty() {
            allowed.push(',');
        }
        allowed.push_str(name);
    }
    allowed
}

/// What is said before a file the server did not make itself, to hold the
/// tool to opening it only as one of `allowed`. Said right before the `-i`
/// the input follows, since it is that one input it speaks for.
pub(crate) fn only_as(allowed: &str) -> [OsString; 2] {
    [
        OsString::from("-format_whitelist"),
        OsString::from(allowed),
    ]
}

/// Puts [`only_as`] right before the one input a reading takes, for a reading
/// whose arguments are built without it. For a command of a single input,
/// which every reading of a picture is.
///
/// An empty set is no set: a build that gave no listing holds the reading to
/// nothing rather than to the empty whitelist, which the tool reads as "no
/// format at all" and would refuse every file for.
pub(crate) fn guard_the_one_input(args: &mut Vec<OsString>, allowed: &str) {
    if allowed.is_empty() {
        return;
    }
    if let Some(at) = args.iter().position(|arg| arg == "-i") {
        let said = only_as(allowed);
        args.splice(at..at, said);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A_LISTING: &str = "\
Demuxers:
 D. = Demuxing supported
 .E = Muxing supported
 --
 D  aac             raw ADTS AAC (Advanced Audio Coding)
 D  concat          Virtual concatenation script
 D  dash            Dynamic Adaptive Streaming over HTTP
 D  flac            raw FLAC
 D  hls             Apple HTTP Live Streaming
 D  matroska,webm   Matroska / WebM
 D  mov,mp4,m4a,3gp,3g2,mj2 QuickTime / MOV
 D  webm_dash_manifest WebM DASH Manifest";

    #[test]
    fn every_reader_but_the_ones_that_open_others_is_allowed() {
        // The tool, given this list, splits it on commas too and matches a
        // file whose own name holds any of these words.
        let allowed = allowed_from(A_LISTING);
        let tokens: Vec<&str> = allowed.split(',').collect();
        for word in ["aac", "flac", "matroska", "webm", "mov", "mp4", "m4a"] {
            assert!(tokens.contains(&word), "{word} in {allowed}");
        }
    }

    #[test]
    fn no_reader_that_opens_other_inputs_is_allowed() {
        let allowed = allowed_from(A_LISTING);
        for dangerous in OPENS_OTHER_INPUTS {
            assert!(
                !allowed.split(',').any(|name| name == dangerous),
                "{dangerous} must not be allowed: {allowed}"
            );
        }
    }

    #[test]
    fn the_guard_is_put_right_before_the_input() {
        let mut args = vec![
            OsString::from("-hide_banner"),
            OsString::from("-i"),
            OsString::from("/films/a.mkv"),
        ];
        guard_the_one_input(&mut args, "matroska,webm");
        let words: Vec<String> =
            args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(
            words,
            ["-hide_banner", "-format_whitelist", "matroska,webm", "-i", "/films/a.mkv"]
        );
    }

    #[test]
    fn an_empty_set_adds_no_guard_rather_than_an_empty_whitelist() {
        // An empty whitelist is read by the tool as "no format at all" and
        // refuses every file, so a build that gave no listing must be held to
        // nothing instead, leaving the reading untouched.
        let mut args = vec![OsString::from("-i"), OsString::from("/films/a.mkv")];
        guard_the_one_input(&mut args, "");
        assert_eq!(args, [OsString::from("-i"), OsString::from("/films/a.mkv")]);
    }

    #[test]
    fn the_lines_about_the_listing_itself_are_not_read_as_readers() {
        let allowed = allowed_from(A_LISTING);
        assert!(!allowed.contains("Demuxing"));
        assert!(!allowed.contains('='));
        assert!(!allowed.is_empty());
    }
}
