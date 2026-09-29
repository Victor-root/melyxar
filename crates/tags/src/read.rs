//! Reading one music file.

use std::error::Error as _;
use std::fs::File;
use std::path::Path;

use lofty::config::ParseOptions;
use lofty::error::FileParseError;
use lofty::file::{AudioFile as _, FileType, TaggedFile, TaggedFileExt};
use lofty::mp4::{Mp4Codec, Mp4File};
use lofty::probe::Probe;
use lofty::properties::FileProperties;
use lofty::tag::{Accessor, ItemKey, Tag};

use crate::values::{cleaned, decibels, names, number, says_yes, year_in};
use crate::{AudioFile, ReadError, Sound, Tags};

/// Reads the tags and the sound of one file.
///
/// Blocking, since it reads the disk: whoever calls it from asynchronous code
/// runs it where waiting on a disk holds nothing else up.
///
/// The pictures a file carries are left where they are. A cover weighs more
/// than everything else in the file put together, and reading every one of
/// them to file a hundred thousand songs would be most of the cost of the
/// scan for pictures that are taken once per album, elsewhere.
pub fn read(path: &Path) -> Result<AudioFile, ReadError> {
    let options = ParseOptions::new().read_cover_art(false);
    let probe = Probe::open(path)
        .map_err(unreadable)?
        .options(options)
        .guess_file_type()
        .map_err(|error| ReadError::Unreadable(error.to_string()))?;
    let Some(file_type) = probe.file_type() else {
        return Err(ReadError::Unsupported);
    };

    // What an MPEG-4 file holds is only said by the file itself: the same
    // `.m4a` is AAC, which every browser plays, or ALAC, which almost none
    // does. The generic reading leaves that out, so this one form is read
    // through its own reader.
    let (tagged, codec) = match file_type {
        FileType::Mp4 => {
            let mut file =
                File::open(path).map_err(|error| ReadError::Unreadable(error.to_string()))?;
            let mp4 = Mp4File::read_from(&mut file, options).map_err(unreadable)?;
            let codec = match mp4.properties().codec() {
                Some(Mp4Codec::ALAC) => "alac",
                Some(Mp4Codec::MP3) => "mp3",
                Some(Mp4Codec::FLAC) => "flac",
                // Whatever else it names, and a file that names nothing, is
                // taken for what nearly every MPEG-4 song is.
                _ => "aac",
            };
            (TaggedFile::from(mp4), codec)
        }
        _ => {
            let tagged = probe.read().map_err(unreadable)?;
            let codec = codec_of(file_type, tagged.properties())?;
            (tagged, codec)
        }
    };

    // The tag the format itself favours, and failing that whatever tag there
    // is: an MP3 carrying only the old short tag still says what it is.
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let tagged_value = |key: ItemKey| tag.and_then(|tag| tag.get_string(key));

    let properties = tagged.properties();
    let sound = Sound {
        container: container_of(file_type)?,
        codec,
        duration_ms: u64::try_from(properties.duration().as_millis()).unwrap_or(u64::MAX),
        overall_bitrate: properties.overall_bitrate().filter(|rate| *rate > 0),
        audio_bitrate: properties.audio_bitrate().filter(|rate| *rate > 0),
        sample_rate: properties.sample_rate().filter(|rate| *rate > 0),
        channels: properties.channels().filter(|count| *count > 0),
        bit_depth: properties.bit_depth().filter(|depth| *depth > 0),
        replay_gain_db: tagged_value(ItemKey::ReplayGainTrackGain).and_then(decibels),
        replay_gain_peak: tagged_value(ItemKey::ReplayGainTrackPeak).and_then(number),
    };
    let tags = tag.map(tags_of).unwrap_or_default();

    Ok(AudioFile { tags, sound })
}

/// What a lofty refusal says, down to its cause: its own wording only names
/// the form it was reading.
fn unreadable(error: FileParseError) -> ReadError {
    if error.is_unknown_format() {
        return ReadError::Unsupported;
    }
    let mut said = error.to_string();
    let mut cause = error.source();
    while let Some(reason) = cause {
        said.push_str(": ");
        said.push_str(&reason.to_string());
        cause = reason.source();
    }
    ReadError::Unreadable(said)
}

/// The container, as the analyser of the films names it.
fn container_of(file_type: FileType) -> Result<&'static str, ReadError> {
    Ok(match file_type {
        FileType::Aac => "aac",
        FileType::Aiff => "aiff",
        FileType::Ape => "ape",
        FileType::Flac => "flac",
        FileType::Mpeg => "mp3",
        FileType::Mp4 => "mov,mp4,m4a,3gp,3g2,mj2",
        FileType::Mpc => "mpc",
        FileType::Opus | FileType::Vorbis | FileType::Speex => "ogg",
        FileType::Wav => "wav",
        FileType::WavPack => "wv",
        _ => return Err(ReadError::Unsupported),
    })
}

/// The codec of every form but MPEG-4, which says it on its own.
///
/// Uncompressed sound is named by its depth, since that is what a player
/// has to know about it; the analyser names it the same way.
fn codec_of(file_type: FileType, properties: &FileProperties) -> Result<&'static str, ReadError> {
    Ok(match file_type {
        FileType::Aac => "aac",
        FileType::Ape => "ape",
        FileType::Flac => "flac",
        FileType::Mpeg => "mp3",
        FileType::Mpc => "musepack",
        FileType::Opus => "opus",
        FileType::Vorbis => "vorbis",
        FileType::Speex => "speex",
        FileType::WavPack => "wavpack",
        FileType::Wav => match properties.bit_depth() {
            Some(8) => "pcm_u8",
            Some(24) => "pcm_s24le",
            Some(32) => "pcm_s32le",
            _ => "pcm_s16le",
        },
        FileType::Aiff => match properties.bit_depth() {
            Some(8) => "pcm_s8",
            Some(24) => "pcm_s24be",
            Some(32) => "pcm_s32be",
            _ => "pcm_s16be",
        },
        _ => return Err(ReadError::Unsupported),
    })
}

/// Reads one tag into the plain values the server files by.
fn tags_of(tag: &Tag) -> Tags {
    let text = |key: ItemKey| tag.get_string(key).and_then(cleaned);

    // A file tagged with care holds every artist in a field of its own and a
    // single display line in the usual one; the separate names are the ones
    // to file by, and the display line is kept for a file that has nothing
    // else.
    let artists = match names(tag.get_strings(ItemKey::TrackArtists)) {
        separate if !separate.is_empty() => separate,
        _ => names(tag.get_strings(ItemKey::TrackArtist)),
    };
    let album_artists = match names(tag.get_strings(ItemKey::AlbumArtists)) {
        separate if !separate.is_empty() => separate,
        _ => names(tag.get_strings(ItemKey::AlbumArtist)),
    };

    Tags {
        title: text(ItemKey::TrackTitle),
        title_sort: text(ItemKey::TrackTitleSortOrder),
        artists,
        artist_sort: text(ItemKey::TrackArtistSortOrder),
        album: text(ItemKey::AlbumTitle),
        album_sort: text(ItemKey::AlbumTitleSortOrder),
        album_artists,
        album_artist_sort: text(ItemKey::AlbumArtistSortOrder),
        track: tag.track().filter(|number| *number > 0),
        track_total: tag.track_total().filter(|number| *number > 0),
        disc: tag.disk().filter(|number| *number > 0),
        disc_total: tag.disk_total().filter(|number| *number > 0),
        year: tag
            .date()
            .map(|date| i32::from(date.year))
            .filter(|year| *year > 0)
            .or_else(|| tag.get_string(ItemKey::Year).and_then(year_in))
            .or_else(|| tag.get_string(ItemKey::RecordingDate).and_then(year_in)),
        genres: names(tag.get_strings(ItemKey::Genre)),
        compilation: tag
            .get_string(ItemKey::FlagCompilation)
            .is_some_and(says_yes),
    }
}
