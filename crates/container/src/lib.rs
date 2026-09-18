//! Reading a film's own index of where its picture can be started.
//!
//! Almost every picture in a film says only what changed since the one before
//! it; every few seconds there is one that stands on its own. Those are the
//! only places a stream carried over untouched can begin, and a container
//! worth the name already knows where they all are: it carries a table of
//! them, which is what lets a desktop player jump into a film the instant it
//! is asked instead of reading the film first.
//!
//! Reading that table costs a few thousand bytes of a file that may be thirty
//! gigabytes. The other way of answering the same question, handing the whole
//! film to the analyser and watching every picture go past, is what made the
//! upkeep of a large library take hours.
//!
//! Nothing here launches a media tool, and nothing here guesses. A container
//! this does not know, an index that is missing, a file still being written,
//! an index built in a shape whose meaning is not certain: all of them answer
//! the same way, with nothing at all. The caller then reads the film through,
//! which is the answer that is always available and always right.

#![forbid(unsafe_code)]

mod isobmff;
mod matroska;
mod reader;

use std::path::Path;

use melyxar_core::time::Millis;

use crate::reader::Reader;

/// How much of the beginning of a file says which family it belongs to.
const ENOUGH_TO_TELL: usize = 8;

/// Reads where a film's picture can be started, from the film's own index.
///
/// Nothing at all means this file carries no index this can answer from, and
/// says nothing whatever about the film: the caller reads it through.
///
/// Off the asynchronous threads, because seeking through a file is waiting on
/// a disk however few bytes it asks for, and a film living on a sleeping disk
/// waits for it to spin up.
pub async fn key_frames(media: &Path) -> Option<Vec<Millis>> {
    let path = media.to_path_buf();
    tokio::task::spawn_blocking(move || read_the_index(&path))
        .await
        .ok()
        .flatten()
}

fn read_the_index(path: &Path) -> Option<Vec<Millis>> {
    let mut reader = Reader::open(path)?;
    let mut head = [0u8; ENOUGH_TO_TELL];
    reader.exactly(&mut head)?;

    let found = if matroska::is_one(&head) {
        matroska::key_frames(&mut reader)?
    } else if isobmff::is_one(&head) {
        isobmff::key_frames(&mut reader)?
    } else {
        return None;
    };

    // An index that named no place at all is not an answer about the film. It
    // is a file whose index this read the shape of and not the meaning of, and
    // saying "this film can be started nowhere" on that would have the film
    // written down as read and never looked at again.
    if found.is_empty() {
        return None;
    }

    // Ascending and each one once, because everything downstream walks them in
    // order and a container is free to list them in any it likes.
    let mut found = found;
    found.sort_unstable();
    found.dedup();
    Some(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tokio::process::Command;

    /// Builds one film, in whatever container the name asks for.
    ///
    /// Made with pictures built from the ones around them and a picture
    /// standing on its own every two seconds, which is the shape of an
    /// ordinary film rather than the shape of something convenient.
    async fn film(directory: &Path, named: &str, extra: &[&str]) -> PathBuf {
        let path = directory.join(named);
        let mut command = Command::new("ffmpeg");
        command.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=24:duration=20",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-g",
            "48",
            "-bf",
            "3",
            "-sc_threshold",
            "0",
        ]);
        command.args(extra);
        let made = command
            .arg(&path)
            .output()
            .await
            .expect("the tool runs here");
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );
        path
    }

    /// What the film says about itself when it is read through, which is the
    /// answer every one of these is measured against.
    async fn by_reading_it_through(path: &Path) -> Vec<Millis> {
        let tools =
            melyxar_ffmpeg::ToolPaths::discover(None, None).expect("the tools are installed here");
        melyxar_ffmpeg::probe::key_frames(&tools.ffprobe, path)
            .await
            .expect("the film is read")
    }

    #[tokio::test]
    async fn every_container_this_knows_answers_exactly_what_the_full_reading_answers() {
        // The whole point of the module in one test. A position half a
        // hundredth out is a segment boundary landing where there is no
        // picture, so "close" is not a result here: it is the same list or it
        // is a fault.
        let directory = tempfile::tempdir().expect("temporary directory");
        let shapes: [(&str, &[&str]); 6] = [
            // The plain ones, one of each family.
            ("plain.mp4", &["-an"]),
            ("plain.mkv", &["-an"]),
            ("plain.mov", &["-an"]),
            (
                "plain.webm",
                &["-an", "-c:v", "libvpx-vp9", "-cpu-used", "8"],
            ),
            // The index moved to the front of the file, which is what a film
            // meant to be played over a network carries.
            ("front.mp4", &["-an", "-movflags", "+faststart"]),
            // Delays written as negative numbers, the newer of the two shapes
            // that table takes.
            (
                "negative.mp4",
                &["-an", "-movflags", "+negative_cts_offsets"],
            ),
        ];

        for (named, extra) in shapes {
            let path = film(directory.path(), named, extra).await;
            let from_its_index = key_frames(&path).await;
            assert_eq!(
                from_its_index.as_deref(),
                Some(by_reading_it_through(&path).await.as_slice()),
                "{named}"
            );
        }
    }

    #[tokio::test]
    async fn a_film_whose_sound_comes_first_still_answers_about_its_picture() {
        // The marks of a Matroska file belong to a track, and a file can carry
        // marks for its sound as well. Answering with those would put the
        // boundaries of the picture wherever the sound could be cut, which is
        // anywhere at all.
        let directory = tempfile::tempdir().expect("temporary directory");
        for (named, sound) in [("sound-first.mkv", "libopus"), ("sound-first.mp4", "aac")] {
            let path = directory.path().join(named);
            let made = Command::new("ffmpeg")
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=size=320x180:rate=24:duration=20",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=440:duration=20",
                    "-map",
                    "1:a",
                    "-map",
                    "0:v",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-g",
                    "48",
                    "-bf",
                    "3",
                    "-sc_threshold",
                    "0",
                    "-c:a",
                    sound,
                    "-shortest",
                ])
                .arg(&path)
                .output()
                .await
                .expect("the tool runs here");
            assert!(
                made.status.success(),
                "{}",
                String::from_utf8_lossy(&made.stderr)
            );

            assert_eq!(
                key_frames(&path).await.as_deref(),
                Some(by_reading_it_through(&path).await.as_slice()),
                "{named}"
            );
        }
    }

    #[tokio::test]
    async fn a_film_carrying_no_index_this_can_read_is_handed_back_rather_than_guessed_at() {
        let directory = tempfile::tempdir().expect("temporary directory");

        // A film written in pieces carries no table of its own: what says
        // where its pictures stand on their own is spread through the film,
        // one note per piece.
        let in_pieces = film(
            directory.path(),
            "pieces.mp4",
            &["-an", "-movflags", "+frag_keyframe+empty_moov"],
        )
        .await;
        assert_eq!(key_frames(&in_pieces).await, None);

        // A container this was never taught.
        let elsewhere = film(directory.path(), "elsewhere.ts", &["-an"]).await;
        assert_eq!(key_frames(&elsewhere).await, None);
    }

    #[tokio::test]
    async fn nothing_that_is_not_a_film_is_ever_read_as_one() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("not-a-film.mkv");

        // Empty, cut short, and a file that opens like one of the two families
        // and holds nothing after that. None of them may answer anything.
        for bytes in [
            [].as_slice(),
            &[0x1A, 0x45, 0xDF, 0xA3],
            &[0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x86, 0x81, 0x01],
            b"\0\0\0\x18ftypisom\0\0\x02\0isomiso2",
        ] {
            std::fs::write(&path, bytes).expect("the file is written");
            assert_eq!(key_frames(&path).await, None, "{bytes:?}");
        }

        assert_eq!(
            key_frames(Path::new("/nowhere/missing.mkv")).await,
            None,
            "a file that is not there answers nothing, and the full reading \
             is what reports why"
        );
    }
}
