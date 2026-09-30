//! Reading real files of every form a collection of music holds.
//!
//! The files under `fixtures` are a second of silence each, carrying no tag at
//! all. A test that needs tags writes them into a copy first, the way a tagging
//! program would, and reads them back.

use std::path::{Path, PathBuf};

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, ItemValue, Tag, TagExt, TagItem};
use melyxar_tags::{ReadError, front_cover, read};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A copy of a fixture, in a folder that goes away with the test.
fn copy_of(name: &str) -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("temporary directory");
    let copy = directory.path().join(name);
    std::fs::copy(fixture(name), &copy).expect("fixture copied");
    (directory, copy)
}

/// Writes a tag into a file, as a tagging program would.
fn tag_file(path: &Path, fill: impl FnOnce(&mut Tag)) {
    let mut tagged = Probe::open(path).expect("opened").read().expect("read");
    let tag_type = tagged.primary_tag_type();
    if tagged.primary_tag().is_none() {
        tagged.insert_tag(Tag::new(tag_type));
    }
    let tag = tagged.primary_tag_mut().expect("a tag to fill");
    fill(tag);
    tag.save_to_path(path, WriteOptions::default())
        .expect("tag written");
}

#[test]
fn every_form_says_how_it_is_packed_and_how_long_it_runs() {
    let expected = [
        ("one-second.mp3", "mp3", "mp3"),
        ("one-second.flac", "flac", "flac"),
        ("one-second.m4a", "mov,mp4,m4a,3gp,3g2,mj2", "aac"),
        ("one-second-alac.m4a", "mov,mp4,m4a,3gp,3g2,mj2", "alac"),
        ("one-second.ogg", "ogg", "vorbis"),
        ("one-second.opus", "ogg", "opus"),
        ("one-second.wv", "wv", "wavpack"),
        ("one-second.wav", "wav", "pcm_s16le"),
    ];
    for (name, container, codec) in expected {
        let file = read(&fixture(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(file.sound.container, container, "{name}");
        assert_eq!(file.sound.codec, codec, "{name}");
        // An MP3 is cut in frames of a fixed length, a seventh of a second at
        // the rate these fixtures use, so it rounds up to the next one.
        assert!(
            (900..=1200).contains(&file.sound.duration_ms),
            "{name} runs for {} ms",
            file.sound.duration_ms
        );
        assert_eq!(file.sound.channels, Some(1), "{name}");
        assert_eq!(file.tags, Default::default(), "{name} carries no tag");
    }
}

#[test]
fn a_form_the_reader_does_not_know_says_so_rather_than_failing() {
    assert!(matches!(
        read(&fixture("one-second.wma")),
        Err(ReadError::Unsupported)
    ));
}

#[test]
fn a_file_that_is_not_music_is_refused() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let fake = directory.path().join("Quiet Harbour.mp3");
    std::fs::write(&fake, b"these are not the sounds you are looking for").expect("written");
    assert!(read(&fake).is_err());
}

#[test]
fn a_missing_file_is_refused() {
    assert!(matches!(
        read(Path::new("/nowhere/at/all.flac")),
        Err(ReadError::Unreadable(_))
    ));
}

#[test]
fn what_a_tagging_program_wrote_is_read_back_in_every_common_form() {
    for name in [
        "one-second.mp3",
        "one-second.flac",
        "one-second.m4a",
        "one-second.ogg",
        "one-second.opus",
        "one-second.wv",
    ] {
        let (_directory, path) = copy_of(name);
        tag_file(&path, |tag| {
            tag.set_title("Quiet Harbour".to_string());
            tag.set_artist("Amber Field".to_string());
            tag.set_album("Northern Lights".to_string());
            tag.insert_text(ItemKey::AlbumArtist, "Amber Field".to_string());
            tag.set_track(3);
            tag.set_track_total(12);
            tag.set_disk(2);
            tag.set_disk_total(2);
            tag.insert_text(ItemKey::RecordingDate, "2019-05-03".to_string());
            tag.set_genre("Folk".to_string());
        });

        let tags = read(&path)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .tags;
        assert_eq!(tags.title.as_deref(), Some("Quiet Harbour"), "{name}");
        assert_eq!(tags.artists, vec!["Amber Field"], "{name}");
        assert_eq!(tags.album.as_deref(), Some("Northern Lights"), "{name}");
        assert_eq!(tags.album_artists, vec!["Amber Field"], "{name}");
        assert_eq!(
            (tags.track, tags.track_total),
            (Some(3), Some(12)),
            "{name}"
        );
        assert_eq!((tags.disc, tags.disc_total), (Some(2), Some(2)), "{name}");
        assert_eq!(tags.year, Some(2019), "{name}");
        assert_eq!(tags.genres, vec!["Folk"], "{name}");
    }
}

#[test]
fn several_artists_are_read_whichever_way_they_were_written() {
    // As several values of the one field, which the newer forms allow.
    let (_first, flac) = copy_of("one-second.flac");
    tag_file(&flac, |tag| {
        tag.push(TagItem::new(
            ItemKey::TrackArtist,
            ItemValue::Text("Amber Field".to_string()),
        ));
        tag.push(TagItem::new(
            ItemKey::TrackArtist,
            ItemValue::Text("The Lanterns".to_string()),
        ));
    });
    assert_eq!(
        read(&flac).expect("read").tags.artists,
        vec!["Amber Field", "The Lanterns"]
    );

    // As one value split by semicolons, which is how the older ones do it.
    let (_second, mp3) = copy_of("one-second.mp3");
    tag_file(&mp3, |tag| {
        tag.set_artist("Amber Field; The Lanterns".to_string())
    });
    assert_eq!(
        read(&mp3).expect("read").tags.artists,
        vec!["Amber Field", "The Lanterns"]
    );

    // And a field made to hold each artist apart wins over the display line.
    let (_third, ogg) = copy_of("one-second.ogg");
    tag_file(&ogg, |tag| {
        tag.set_artist("Amber Field feat. The Lanterns".to_string());
        tag.push(TagItem::new(
            ItemKey::TrackArtists,
            ItemValue::Text("Amber Field".to_string()),
        ));
        tag.push(TagItem::new(
            ItemKey::TrackArtists,
            ItemValue::Text("The Lanterns".to_string()),
        ));
    });
    assert_eq!(
        read(&ogg).expect("read").tags.artists,
        vec!["Amber Field", "The Lanterns"]
    );
}

#[test]
fn a_compilation_and_how_names_are_sorted_are_read() {
    let (_directory, path) = copy_of("one-second.m4a");
    tag_file(&path, |tag| {
        tag.insert_text(ItemKey::FlagCompilation, "1".to_string());
        tag.insert_text(ItemKey::TrackArtistSortOrder, "Field, Amber".to_string());
        tag.insert_text(ItemKey::AlbumTitleSortOrder, "Lights, Northern".to_string());
    });
    let tags = read(&path).expect("read").tags;
    assert!(tags.compilation);
    assert_eq!(tags.artist_sort.as_deref(), Some("Field, Amber"));
    assert_eq!(tags.album_sort.as_deref(), Some("Lights, Northern"));
}

#[test]
fn a_year_alone_is_read_as_well_as_a_full_date() {
    let (_directory, path) = copy_of("one-second.flac");
    tag_file(&path, |tag| {
        tag.insert_text(ItemKey::Year, "1987".to_string());
    });
    assert_eq!(read(&path).expect("read").tags.year, Some(1987));
}

#[test]
fn the_cover_a_file_carries_is_taken_out_as_it_was_stored() {
    use lofty::picture::{MimeType, Picture, PictureType};

    let (_directory, path) = copy_of("one-second.mp3");
    assert_eq!(front_cover(&path).expect("read"), None, "no picture yet");

    let front = b"\x89PNG not really a picture, but bytes kept as they are".to_vec();
    tag_file(&path, |tag| {
        tag.push_picture(
            Picture::unchecked(b"the back of the sleeve".to_vec())
                .pic_type(PictureType::CoverBack)
                .mime_type(MimeType::Jpeg)
                .build(),
        );
        tag.push_picture(
            Picture::unchecked(front.clone())
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Png)
                .build(),
        );
    });
    let cover = front_cover(&path).expect("read").expect("a cover");
    assert_eq!(cover.data, front, "the front, not the back");
    assert_eq!(cover.extension, "png");

    // And reading the tags never loads it.
    assert_eq!(read(&path).expect("read").tags, Default::default());
}

#[test]
fn a_picture_marked_as_nothing_in_particular_is_the_cover_all_the_same() {
    use lofty::picture::{MimeType, Picture, PictureType};

    let (_directory, path) = copy_of("one-second.flac");
    tag_file(&path, |tag| {
        tag.push_picture(
            Picture::unchecked(b"a picture".to_vec())
                .pic_type(PictureType::Other)
                .mime_type(MimeType::Jpeg)
                .build(),
        );
    });
    let cover = front_cover(&path).expect("read").expect("a cover");
    assert_eq!(cover.data, b"a picture");
    assert_eq!(cover.extension, "jpg");
}

#[test]
fn the_gain_a_tagging_program_measured_is_read_with_the_sound() {
    let (_directory, song) = copy_of("one-second.flac");
    tag_file(&song, |tag| {
        tag.insert_text(ItemKey::ReplayGainTrackGain, "-6.54 dB".to_string());
        tag.insert_text(ItemKey::ReplayGainTrackPeak, "0.988".to_string());
    });
    let tagged = read(&song).expect("read");
    assert_eq!(tagged.sound.replay_gain_db, Some(-6.54));
    assert_eq!(tagged.sound.replay_gain_peak, Some(0.988));

    let (_other, untagged) = copy_of("one-second.mp3");
    assert_eq!(read(&untagged).expect("read").sound.replay_gain_db, None);
}

#[test]
fn what_the_tag_manager_writes_is_read_back_and_the_sound_is_untouched() {
    for name in [
        "one-second.flac",
        "one-second.mp3",
        "one-second.m4a",
        "one-second.ogg",
        "one-second.opus",
    ] {
        let (_directory, song) = copy_of(name);
        tag_file(&song, |tag| {
            tag.insert_text(ItemKey::TrackTitle, "Old title".to_string());
            tag.insert_text(ItemKey::Genre, "Old genre".to_string());
        });
        let before = read(&song).expect("read").sound;
        let edited = melyxar_tags::EditedTags {
            title: Some("Quiet Harbour".to_string()),
            artists: vec!["Amber Field".to_string(), "The Lanterns".to_string()],
            album: Some("Northern Lights".to_string()),
            album_artists: vec!["Amber Field".to_string()],
            track: Some(3),
            disc: Some(2),
            year: Some(2019),
            genres: Vec::new(),
            compilation: false,
        };
        melyxar_tags::write(&song, &edited).expect("written");

        let after = read(&song).expect("read again");
        assert_eq!(after.tags.title.as_deref(), Some("Quiet Harbour"), "{name}");
        assert_eq!(
            after.tags.artists,
            vec!["Amber Field", "The Lanterns"],
            "{name}"
        );
        assert_eq!(
            after.tags.album.as_deref(),
            Some("Northern Lights"),
            "{name}"
        );
        assert_eq!(after.tags.album_artists, vec!["Amber Field"], "{name}");
        assert_eq!(after.tags.track, Some(3), "{name}");
        assert_eq!(after.tags.disc, Some(2), "{name}");
        assert_eq!(after.tags.year, Some(2019), "{name}");
        assert!(
            after.tags.genres.is_empty(),
            "{name}: a genre chosen away is gone"
        );
        assert_eq!(after.sound.duration_ms, before.duration_ms, "{name}");
        assert_eq!(after.sound.codec, before.codec, "{name}");
    }
}

#[test]
fn the_cover_a_file_carries_is_still_there_once_its_tags_are_written() {
    use lofty::picture::{MimeType, Picture, PictureType};

    let (_directory, song) = copy_of("one-second.flac");
    tag_file(&song, |tag| {
        tag.push_picture(
            Picture::unchecked(b"the front".to_vec())
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Jpeg)
                .build(),
        );
    });
    melyxar_tags::write(
        &song,
        &melyxar_tags::EditedTags {
            title: Some("Tides".to_string()),
            ..Default::default()
        },
    )
    .expect("written");
    let cover = front_cover(&song).expect("read").expect("still a cover");
    assert_eq!(cover.data, b"the front");
    assert_eq!(
        read(&song).expect("read").tags.title.as_deref(),
        Some("Tides")
    );
}
