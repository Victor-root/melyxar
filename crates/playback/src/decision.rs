//! Deciding how a file reaches a viewer.
//!
//! A pure function: the same inputs always give the same answer, and nothing
//! here reads a disk, a database or a clock. That is what lets every branch be
//! covered by a test in milliseconds, and it is what makes the answer
//! explainable, because the reasons are produced alongside it.
//!
//! Those reasons are not a nicety. Without them, every "why is this
//! transcoding?" becomes a debugging session, so each one is a structured
//! value a client can translate rather than a sentence baked into the server.

use melyxar_core::media::{
    AudioDetails, HdrFormat, MediaSource, SubtitleDetails, Track, TrackKind, VideoDetails,
};
use melyxar_core::user::{DownmixMethod, WideGamutChoice};
use serde::{Deserialize, Serialize};

use crate::profile::ClientProfile;

/// How the stream is produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackMethod {
    /// The file is served as it is and the client opens it. Nothing is
    /// decoded server side.
    DirectPlay,
    /// Streams are copied into another container. No decoding, so the cost is
    /// close to nothing.
    Remux,
    /// Video copied, audio rebuilt. The common case for a personal library:
    /// the picture is fine, the sound is not.
    TranscodeAudio,
    /// Everything decoded and rebuilt. The expensive path.
    FullTranscode,
}

impl PlaybackMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DirectPlay => "direct_play",
            Self::Remux => "remux",
            Self::TranscodeAudio => "transcode_audio",
            Self::FullTranscode => "full_transcode",
        }
    }

    /// Whether this method counts against the transcoding limit.
    ///
    /// Copying costs almost nothing, so remuxing sessions are not capped.
    pub fn is_expensive(self) -> bool {
        matches!(self, Self::TranscodeAudio | Self::FullTranscode)
    }
}

/// Why the decision came out the way it did.
///
/// A code plus its values, never a finished sentence: the client owns the
/// wording, which is what lets every client translate it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Reason {
    /// Everything lined up and the file is served untouched.
    EverythingSupported,
    ContainerNotSupported {
        container: String,
    },
    VideoCodecNotSupported {
        codec: String,
    },
    VideoProfileNotSupported {
        codec: String,
        profile: Option<String>,
    },
    ResolutionTooHigh {
        height: i32,
        max_height: i32,
    },
    /// Wide gamut colour the client cannot show. Left alone it looks washed
    /// out and grey, so it is converted.
    WideGamutNotSupported {
        format: HdrFormat,
    },
    /// This flavour carries no compatible base layer: played untouched it
    /// shows green and purple rather than a picture.
    DolbyVisionWithoutBaseLayer {
        profile: Option<i32>,
    },
    /// Wide gamut colour the viewer asked to have converted.
    WideGamutConversionChosen {
        format: HdrFormat,
    },
    /// The picture is rebuilt for another reason, and a rebuilt picture is of
    /// standard range: its wide gamut colour is converted on the way.
    WideGamutRebuiltAsStandard {
        format: HdrFormat,
    },
    InterlacedPicture,
    BitrateTooHigh {
        bitrate: i64,
        max_bitrate: i64,
    },
    AudioCodecNotSupported {
        codec: String,
    },
    TooManyAudioChannels {
        channels: i32,
        max_channels: i32,
    },
    /// A fold to stereo was asked for. This is what makes the preference a
    /// real input of the decision rather than a setting that quietly does
    /// nothing on files the client could play as they are.
    DownmixRequested {
        method: DownmixMethod,
    },
    /// Levelling the loudness means touching the audio.
    LoudnessLevellingRequested,
    /// Picture subtitles can only be burnt into the video.
    SubtitleMustBeBurnedIn {
        codec: String,
    },
    /// The client draws no subtitle format this server can deliver alongside
    /// the picture, so the only way to show one is to draw it in.
    SubtitleFormatNotDrawnByClient {
        format: String,
    },
    /// The client cannot switch tracks inside a file it plays directly, so
    /// choosing another one means rebuilding the stream.
    NonDefaultTrackSelected,
}

/// What to do with each stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamAction {
    /// Leave the stream out.
    Drop,
    /// Carry it over untouched.
    Copy,
    /// Rebuild it.
    Transcode,
}

/// How subtitles reach the viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleDelivery {
    None,
    /// Converted and delivered next to the video, which the client draws.
    /// Cheap, and the viewer can turn it off.
    External,
    /// Drawn into the picture. Forces a full transcode and cannot be turned
    /// off during playback.
    BurnIn,
}

/// The answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackDecision {
    pub method: PlaybackMethod,
    pub video: StreamAction,
    pub audio: StreamAction,
    pub subtitles: SubtitleDelivery,
    /// Stream index of the chosen audio track inside the file.
    pub audio_stream_index: Option<i32>,
    pub subtitle_stream_index: Option<i32>,
    pub video_stream_index: Option<i32>,
    /// Height to scale down to, when the client asked for a smaller picture.
    pub scale_to_height: Option<i32>,
    /// Rate the rebuilt picture must stay under, when the client asked for
    /// one. Absent when it did not, or when nothing is being rebuilt.
    ///
    /// Carried on the answer rather than read from the profile again later:
    /// asking for a rate is what turns a film into a rebuild in the first
    /// place, and producing it at some other rate would be the cost of the
    /// work without the point of it.
    pub bitrate_ceiling: Option<i64>,
    /// Convert wide gamut colour to standard range.
    pub tone_map: bool,
    /// Whether the viewer's choice decided what is done with the picture's
    /// wide gamut colour. False for a picture of standard range, and for one
    /// whose handling was imposed: a flavour always converted, a picture
    /// rebuilt anyway, a server told never to convert.
    pub wide_gamut_follows_choice: bool,
    /// Every reason behind the answer, in the order they were found.
    pub reasons: Vec<Reason>,
}

impl PlaybackDecision {
    /// Whether the answer is the cheapest possible one.
    pub fn is_direct_play(&self) -> bool {
        self.method == PlaybackMethod::DirectPlay
    }
}

/// What the viewer asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackRequest<'a> {
    pub profile: &'a ClientProfile,
    /// Chosen audio track. Absent means the one the file marks as default.
    pub audio_track: Option<&'a Track>,
    /// Chosen subtitle track. Absent means none.
    pub subtitle_track: Option<&'a Track>,
    /// How multichannel audio should be folded down.
    pub downmix: DownmixMethod,
    /// Whether loudness levelling was asked for and a measurement exists.
    pub level_loudness: bool,
    /// Never convert wide gamut colour, whatever the client's own profile
    /// says about it.
    ///
    /// The operator's own switch, made for the weight of the conversion
    /// rather than for the picture: on a processor too slow to keep up with
    /// it, this turns what would otherwise be a full rebuild into a copy, for
    /// every film whose only reason to rebuild was its colour. Such a film
    /// then looks washed out and grey rather than correct, which is the whole
    /// of what is being traded away.
    ///
    /// **Never reaches Dolby Vision without a compatible base layer.** Left
    /// unconverted that looks broken rather than merely washed out, so it is
    /// converted regardless of this.
    pub never_tone_map: bool,
    /// What the viewer wants done with wide gamut colour, for a picture that
    /// could otherwise be carried over as it is.
    pub wide_gamut: WideGamutChoice,
}

/// Decides how a source reaches a client.
pub fn decide(source: &MediaSource, request: &PlaybackRequest<'_>) -> PlaybackDecision {
    let mut reasons = Vec::new();

    let video = source.primary_video();
    let audio = chosen_audio(source, request.audio_track);
    let subtitle = chosen_subtitle(request.subtitle_track);

    let video_action = decide_video(video, source, request.profile, &mut reasons);
    let audio_action = decide_audio(audio, request, &mut reasons);
    let (delivery, subtitle_forces_burn) =
        decide_subtitles(subtitle, request.profile, &mut reasons);

    // Burning subtitles in means decoding and redrawing the picture, whatever
    // the video would otherwise have needed.
    let video_action = if subtitle_forces_burn {
        StreamAction::Transcode
    } else {
        video_action
    };

    // The colour last, because whether the picture is rebuilt anyway is part
    // of the answer: a picture rebuilt for any reason comes out of standard
    // range, and can only keep its colours by having them converted.
    let colour = video.map_or(Colour::Standard, |(_, details)| {
        wide_gamut_colour(details, request, video_action == StreamAction::Transcode)
    });
    let (conversion, wide_gamut_follows_choice) = match colour {
        Colour::Standard => (None, false),
        Colour::Imposed(conversion) => (conversion, false),
        Colour::Chosen(conversion) => (conversion, true),
    };
    let tone_map = conversion.is_some();
    let video_action = match conversion {
        Some(reason) => {
            reasons.push(reason);
            StreamAction::Transcode
        }
        None => video_action,
    };

    let scale_to_height = match (request.profile.max_height, video) {
        (Some(max), Some((_, details))) if details.visible_height() > max => Some(max),
        _ => None,
    };

    // The container only matters when nothing else already forces a rebuild.
    let container = effective_container(source);
    let container_supported = container
        .as_deref()
        .is_some_and(|container| request.profile.supports_container(container));

    // Picking a track other than the default means the client would have to
    // switch inside the container, which browsers cannot do.
    let track_choice_forces_rebuild = !request.profile.can_switch_tracks_in_container
        && chose_a_non_default_track(source, audio, request);

    let method = if video_action == StreamAction::Transcode {
        PlaybackMethod::FullTranscode
    } else if audio_action == StreamAction::Transcode {
        PlaybackMethod::TranscodeAudio
    } else if !container_supported
        || track_choice_forces_rebuild
        || delivery != SubtitleDelivery::None
    {
        if !container_supported {
            if let Some(container) = &container {
                reasons.push(Reason::ContainerNotSupported {
                    container: container.clone(),
                });
            }
        }
        if track_choice_forces_rebuild {
            reasons.push(Reason::NonDefaultTrackSelected);
        }
        PlaybackMethod::Remux
    } else {
        PlaybackMethod::DirectPlay
    };

    if method == PlaybackMethod::DirectPlay && reasons.is_empty() {
        reasons.push(Reason::EverythingSupported);
    }

    // Only when something is actually being rebuilt: a film handed over as it
    // lies on the disk arrives at the rate it was written at, whatever anybody
    // asked for.
    let bitrate_ceiling = match video_action {
        StreamAction::Transcode => request.profile.max_bitrate,
        _ => None,
    };

    PlaybackDecision {
        method,
        // Direct play needs no special case here: when there is a picture and
        // nothing to rebuild, the decision above is already to carry it over,
        // and a file that holds no picture must not be described as carrying
        // one.
        video: video_action,
        audio: audio_action,
        subtitles: delivery,
        audio_stream_index: audio.map(|(track, _)| track.stream_index),
        subtitle_stream_index: subtitle.map(|(track, _)| track.stream_index),
        video_stream_index: video.map(|(track, _)| track.stream_index),
        scale_to_height,
        bitrate_ceiling,
        tone_map,
        wide_gamut_follows_choice,
        reasons,
    }
}

/// The container a client would actually see, as one name.
///
/// The analyser names a container by every format its demuxer handles, so one
/// file comes back as a comma separated family. That is a real trap here: the
/// family shared by the open container and the web container contains a name
/// browsers support and a name they do not, so matching any member would call
/// a file directly playable when it is not.
///
/// The file extension settles it, because that is what the browser itself goes
/// by. The reported name is only a fallback for a file with no extension.
pub fn effective_container(source: &MediaSource) -> Option<String> {
    let extension = source
        .relative_path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_lowercase);

    if let Some(extension) = extension {
        let known = matches!(
            extension.as_str(),
            "mkv"
                | "webm"
                | "mp4"
                | "m4v"
                | "mov"
                | "avi"
                | "ts"
                | "m2ts"
                | "mts"
                | "flv"
                | "wmv"
                | "ogv"
                | "mpg"
                | "mpeg"
                | "m4a"
                | "mp3"
                | "flac"
                | "opus"
                | "ogg"
                | "wav"
        );
        if known {
            // The open container is spelled out in full, because that is the
            // name a profile lists.
            return Some(if extension == "mkv" {
                "matroska".to_string()
            } else {
                extension
            });
        }
    }

    // No usable extension: fall back to the first reported name.
    source
        .container
        .as_deref()
        .and_then(|value| value.split(',').next())
        .map(|value| value.trim().to_string())
}

/// The audio track that will be used: the chosen one, else the default, else
/// the first present.
///
/// An audio description is never reached for on its own, even when the file
/// marks it as its default: it is a narrator talking over the film, wanted by
/// the few who ask for it and by nobody who did not. Asked for by hand it is
/// played like any other track.
fn chosen_audio<'a>(
    source: &'a MediaSource,
    requested: Option<&'a Track>,
) -> Option<(&'a Track, &'a AudioDetails)> {
    if let Some(track) = requested {
        if let TrackKind::Audio(details) = &track.kind {
            return Some((track, details));
        }
    }
    let on_its_own = || {
        source
            .audio_tracks()
            .filter(|(track, _)| !track.is_audio_description())
    };
    on_its_own()
        .find(|(track, _)| track.is_default)
        .or_else(|| on_its_own().next())
        .or_else(|| source.audio_tracks().find(|(track, _)| track.is_default))
        .or_else(|| source.audio_tracks().next())
}

fn chosen_subtitle(requested: Option<&Track>) -> Option<(&Track, &SubtitleDetails)> {
    let track = requested?;
    match &track.kind {
        TrackKind::Subtitle(details) => Some((track, details)),
        _ => None,
    }
}

/// Whether the chosen track is one a browser left to itself would not show.
///
/// "Left to itself" turned out to mean something narrower than the file's own
/// idea of a default. A real file marked its French track the one to default
/// to, and a real browser, handed the file whole with nothing telling it
/// otherwise, played its English track regardless: whatever a browser shows
/// unhelped is the first stream in the file, disposition or no disposition,
/// because nothing here has ever asked one to read that flag and nothing
/// proves it does. So what counts as "what the file would play on its own"
/// here is the first track, and reaching for anything else, automatically or
/// by hand, costs a rebuild that hands over exactly the one track wanted and
/// leaves a browser nothing to guess at.
fn chose_a_non_default_track(
    source: &MediaSource,
    audio: Option<(&Track, &AudioDetails)>,
    request: &PlaybackRequest<'_>,
) -> bool {
    if request.subtitle_track.is_some() {
        return true;
    }
    let Some((chosen, _)) = audio else {
        return false;
    };
    let first_index = source
        .audio_tracks()
        .next()
        .map(|(track, _)| track.stream_index);
    Some(chosen.stream_index) != first_index
}

/// What is done with a picture's wide gamut colour: the conversion when it is
/// converted, with its reason, and nothing when it is kept.
enum Colour {
    /// A picture of standard range, with nothing to decide.
    Standard,
    /// Decided whatever the viewer chose.
    Imposed(Option<Reason>),
    /// Decided by the viewer's choice.
    Chosen(Option<Reason>),
}

/// What is done with this stream's wide gamut colour, and why.
///
/// Four rules, in this order. Dolby Vision without a compatible base layer is
/// always converted: left alone it looks broken rather than washed out, on
/// any screen, and no switch may leave that on one. The operator's switch
/// then converts nothing else. A picture rebuilt for another reason is
/// converted, since what comes out of a rebuild is of standard range. Only a
/// picture that could be carried over as it is is left to the viewer's
/// choice, which by default keeps it where the client shows it.
fn wide_gamut_colour(
    details: &VideoDetails,
    request: &PlaybackRequest<'_>,
    rebuilt_anyway: bool,
) -> Colour {
    let Some(format) = details.hdr else {
        return Colour::Standard;
    };
    if let HdrFormat::DolbyVision { profile } = format {
        if format.is_incompatible_without_conversion() {
            return Colour::Imposed(Some(Reason::DolbyVisionWithoutBaseLayer { profile }));
        }
    }
    if request.never_tone_map {
        return Colour::Imposed(None);
    }
    if rebuilt_anyway {
        return Colour::Imposed(Some(Reason::WideGamutRebuiltAsStandard { format }));
    }
    Colour::Chosen(match request.wide_gamut {
        WideGamutChoice::NeverConvert => None,
        WideGamutChoice::AlwaysConvert => Some(Reason::WideGamutConversionChosen { format }),
        WideGamutChoice::Automatic => (!request
            .profile
            .shows_wide_gamut(&details.codec, format))
        .then_some(Reason::WideGamutNotSupported { format }),
    })
}

fn decide_video(
    video: Option<(&Track, &VideoDetails)>,
    source: &MediaSource,
    profile: &ClientProfile,
    reasons: &mut Vec<Reason>,
) -> StreamAction {
    let Some((_, details)) = video else {
        return StreamAction::Drop;
    };

    let mut must_rebuild = false;

    if !profile.supports_video(&details.codec, details.profile.as_deref(), details.level) {
        // Told apart so that the reason is useful: an unsupported codec and a
        // supported codec in an unsupported profile call for different advice.
        let codec_known = profile
            .video
            .iter()
            .any(|capability| capability.codec.eq_ignore_ascii_case(&details.codec));
        if codec_known {
            reasons.push(Reason::VideoProfileNotSupported {
                codec: details.codec.clone(),
                profile: details.profile.clone(),
            });
        } else {
            reasons.push(Reason::VideoCodecNotSupported {
                codec: details.codec.clone(),
            });
        }
        must_rebuild = true;
    }

    if let Some(max) = profile.max_height {
        if details.visible_height() > max {
            reasons.push(Reason::ResolutionTooHigh {
                height: details.visible_height(),
                max_height: max,
            });
            must_rebuild = true;
        }
    }

    if details.is_interlaced {
        reasons.push(Reason::InterlacedPicture);
        must_rebuild = true;
    }

    // The rate of the picture when the file states one, otherwise the rate of
    // the whole file. The second is the sound as well and is therefore a
    // little high, but most films in a collection state no rate per stream at
    // all, and a limit that quietly does nothing on most films is worse than
    // no limit: somebody would set it, see no change, and conclude the setting
    // is broken.
    let arriving_at = details.bitrate.or(source.overall_bitrate);
    if let (Some(max), Some(bitrate)) = (profile.max_bitrate, arriving_at) {
        if bitrate > max {
            reasons.push(Reason::BitrateTooHigh {
                bitrate,
                max_bitrate: max,
            });
            must_rebuild = true;
        }
    }

    if must_rebuild {
        StreamAction::Transcode
    } else {
        StreamAction::Copy
    }
}

fn decide_audio(
    audio: Option<(&Track, &AudioDetails)>,
    request: &PlaybackRequest<'_>,
    reasons: &mut Vec<Reason>,
) -> StreamAction {
    let Some((_, details)) = audio else {
        return StreamAction::Drop;
    };

    let mut must_rebuild = false;

    if !request.profile.supports_audio_codec(&details.codec) {
        reasons.push(Reason::AudioCodecNotSupported {
            codec: details.codec.clone(),
        });
        must_rebuild = true;
    }

    if let Some(max) = request.profile.max_audio_channels {
        if details.channels > max {
            reasons.push(Reason::TooManyAudioChannels {
                channels: details.channels,
                max_channels: max,
            });
            must_rebuild = true;
        }
    }

    // A fold has to be applied by the server, so asking for one means the
    // audio is rebuilt even when the client could have played it untouched.
    // Without this the setting would silently do nothing on such files, which
    // is worse than not offering it.
    if request.downmix.requires_processing() && details.is_multichannel() {
        reasons.push(Reason::DownmixRequested {
            method: request.downmix,
        });
        must_rebuild = true;
    }

    if request.level_loudness && details.loudness.is_measured() {
        reasons.push(Reason::LoudnessLevellingRequested);
        must_rebuild = true;
    }

    if must_rebuild {
        StreamAction::Transcode
    } else {
        StreamAction::Copy
    }
}

/// The format text subtitles are converted to before being sent alongside the
/// picture. One format rather than several: it is the one every browser draws.
const DELIVERED_SUBTITLE_FORMAT: &str = "webvtt";

/// Decides how subtitles are delivered, and says whether that forces the
/// picture to be rebuilt.
fn decide_subtitles(
    subtitle: Option<(&Track, &SubtitleDetails)>,
    profile: &ClientProfile,
    reasons: &mut Vec<Reason>,
) -> (SubtitleDelivery, bool) {
    let Some((_, details)) = subtitle else {
        return (SubtitleDelivery::None, false);
    };

    if details.forces_full_transcode() {
        reasons.push(Reason::SubtitleMustBeBurnedIn {
            codec: details.codec.clone(),
        });
        return (SubtitleDelivery::BurnIn, true);
    }

    // Text subtitles are converted to the one format clients draw and sent
    // alongside, which keeps them switchable during playback. A client that
    // draws none has to have them burnt in: sending a track it cannot draw
    // means a viewer who asked for subtitles and sees none.
    if profile.supports_subtitle_format(DELIVERED_SUBTITLE_FORMAT) {
        return (SubtitleDelivery::External, false);
    }

    reasons.push(Reason::SubtitleFormatNotDrawnByClient {
        format: DELIVERED_SUBTITLE_FORMAT.to_string(),
    });
    (SubtitleDelivery::BurnIn, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::id::{LibraryRootId, MediaSourceId, TrackId, WorkId};
    use crate::profile::{VideoCapability, WideGamutCapability};
    use melyxar_core::media::{
        ColorInfo, Curve, FileIdentity, Loudness, SubtitleLayout, VideoDetails,
    };
    use melyxar_core::time::{now, Millis};
    use std::path::PathBuf;

    fn video_track(index: i32, codec: &str, height: i32, hdr: Option<HdrFormat>) -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: index,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Video(VideoDetails {
                codec: codec.to_string(),
                profile: Some("High".into()),
                level: Some(40),
                width: height * 16 / 9,
                height,
                margins: None,
                aspect_ratio: None,
                is_interlaced: false,
                frame_rate: Some(24.0),
                bitrate: None,
                pixel_format: None,
                reference_frames: None,
                color: ColorInfo::default(),
                hdr,
            }),
        }
    }

    fn audio_track(index: i32, codec: &str, channels: i32, is_default: bool) -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: index,
            language: Some("fre".into()),
            title: None,
            is_default,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: codec.to_string(),
                profile: None,
                channels,
                channel_layout: None,
                sample_rate: Some(48_000),
                bit_depth: None,
                bitrate: None,
                loudness: Loudness::default(),
            }),
        }
    }

    fn subtitle_track(index: i32, codec: &str, layout: SubtitleLayout) -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: index,
            language: Some("fre".into()),
            title: None,
            is_default: false,
            is_forced: false,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: codec.to_string(),
                layout,
                is_hearing_impaired: false,
                is_external: false,
                external_relative_path: None,
            }),
        }
    }

    fn source_named(name: &str, container: &str, tracks: Vec<Track>) -> MediaSource {
        MediaSource {
            id: MediaSourceId::new(),
            work_id: WorkId::new(),
            root_id: LibraryRootId::new(),
            relative_path: PathBuf::from(name),
            container: Some(container.to_string()),
            duration: Some(Millis::new(6_000_000)),
            overall_bitrate: None,
            identity: FileIdentity {
                size_bytes: 1,
                modified_at: now(),
                content_fingerprint: None,
            },
            added_at: now(),
            tracks,
        }
    }

    /// A source whose file name matches the reported container family, which
    /// is the ordinary case.
    fn source(container: &str, tracks: Vec<Track>) -> MediaSource {
        let name = match container {
            c if c.starts_with("matroska") => "film.mkv",
            c if c.starts_with("mov") || c.starts_with("mp4") => "film.mp4",
            _ => "film.bin",
        };
        source_named(name, container, tracks)
    }

    fn request<'a>(profile: &'a ClientProfile) -> PlaybackRequest<'a> {
        PlaybackRequest {
            profile,
            audio_track: None,
            subtitle_track: None,
            downmix: DownmixMethod::None,
            level_loudness: false,
            never_tone_map: false,
            wide_gamut: WideGamutChoice::Automatic,
        }
    }

    #[test]
    fn a_file_the_browser_can_open_is_served_untouched() {
        let profile = ClientProfile::conservative_browser();
        let source = source(
            "mov,mp4,m4a",
            vec![
                video_track(0, "h264", 1080, None),
                audio_track(1, "aac", 2, true),
            ],
        );
        let decision = decide(&source, &request(&profile));

        assert_eq!(decision.method, PlaybackMethod::DirectPlay);
        assert!(decision.is_direct_play());
        assert_eq!(decision.video, StreamAction::Copy);
        assert_eq!(decision.audio, StreamAction::Copy);
        assert_eq!(decision.reasons, vec![Reason::EverythingSupported]);
        assert!(!decision.method.is_expensive());
    }

    #[test]
    fn a_container_the_browser_cannot_open_is_only_repackaged() {
        let profile = ClientProfile::conservative_browser();
        let source = source(
            "matroska,webm",
            vec![
                video_track(0, "h264", 1080, None),
                audio_track(1, "aac", 2, true),
            ],
        );
        let decision = decide(&source, &request(&profile));

        assert_eq!(decision.method, PlaybackMethod::Remux);
        assert_eq!(decision.video, StreamAction::Copy);
        assert_eq!(decision.audio, StreamAction::Copy);
        assert!(!decision.method.is_expensive(), "copying is nearly free");
        assert!(decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::ContainerNotSupported { .. })));
    }

    #[test]
    fn the_common_case_of_a_personal_library_keeps_the_picture_and_rebuilds_the_sound() {
        // The shape most films in a collection actually have: a container the
        // browser will not open and a soundtrack it cannot decode.
        let profile = ClientProfile::conservative_browser();
        let source = source(
            "matroska,webm",
            vec![
                video_track(0, "h264", 1080, None),
                audio_track(1, "eac3", 6, true),
            ],
        );
        let decision = decide(&source, &request(&profile));

        assert_eq!(decision.method, PlaybackMethod::TranscodeAudio);
        assert_eq!(decision.video, StreamAction::Copy, "the picture is fine");
        assert_eq!(decision.audio, StreamAction::Transcode);
        assert!(decision.method.is_expensive());
        assert!(decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::AudioCodecNotSupported { .. })));
        assert!(decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::TooManyAudioChannels { .. })));
    }

    #[test]
    fn a_codec_the_browser_cannot_decode_forces_the_expensive_path() {
        let profile = ClientProfile::conservative_browser();
        let source = source(
            "matroska,webm",
            vec![
                video_track(0, "hevc", 2160, None),
                audio_track(1, "aac", 2, true),
            ],
        );
        let decision = decide(&source, &request(&profile));

        assert_eq!(decision.method, PlaybackMethod::FullTranscode);
        assert_eq!(decision.video, StreamAction::Transcode);
        assert!(decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::VideoCodecNotSupported { .. })));
    }

    #[test]
    fn wide_gamut_colour_is_converted_for_a_client_that_does_not_show_it() {
        let profile = ClientProfile::conservative_browser();
        let source = source(
            "matroska,webm",
            vec![
                video_track(0, "h264", 2160, Some(HdrFormat::Hdr10)),
                audio_track(1, "aac", 2, true),
            ],
        );
        let decision = decide(&source, &request(&profile));

        assert_eq!(decision.method, PlaybackMethod::FullTranscode);
        assert!(decision.tone_map, "otherwise the picture looks washed out");
        assert!(decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::WideGamutNotSupported { .. })));
    }

    #[test]
    fn the_operators_own_switch_skips_the_conversion_a_slow_processor_cannot_afford() {
        // The reason this switch exists: on a processor too slow to rebuild
        // film after film, the automatic rule alone can mean nothing plays at
        // a usable speed. Asked to skip it, a film whose only reason to
        // rebuild was its colour is carried over untouched instead.
        let profile = ClientProfile::conservative_browser();
        let source = source(
            "matroska,webm",
            vec![
                video_track(0, "h264", 2160, Some(HdrFormat::Hdr10)),
                audio_track(1, "aac", 2, true),
            ],
        );
        let decision = decide(
            &source,
            &PlaybackRequest {
                never_tone_map: true,
                ..request(&profile)
            },
        );

        assert_eq!(decision.video, StreamAction::Copy);
        assert!(!decision.tone_map, "nothing is being converted any more");
        assert!(!decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::WideGamutNotSupported { .. })));
    }

    #[test]
    fn the_operators_switch_never_reaches_the_flavour_that_looks_broken_unconverted() {
        // Dolby Vision without a compatible base layer is not merely washed
        // out when left alone, it is green and purple. No switch may leave
        // that on a screen, whatever it was asked to skip.
        let profile = ClientProfile::conservative_browser();
        let source = source(
            "matroska,webm",
            vec![
                video_track(
                    0,
                    "h264",
                    2160,
                    Some(HdrFormat::DolbyVision { profile: Some(5) }),
                ),
                audio_track(1, "aac", 2, true),
            ],
        );
        let decision = decide(
            &source,
            &PlaybackRequest {
                never_tone_map: true,
                ..request(&profile)
            },
        );

        assert_eq!(decision.method, PlaybackMethod::FullTranscode);
        assert_eq!(decision.video, StreamAction::Transcode);
        assert!(decision.tone_map, "converted regardless of the switch");
        assert!(decision.reasons.iter().any(|reason| matches!(
            reason,
            Reason::DolbyVisionWithoutBaseLayer { profile: Some(5) }
        )));
    }

    #[test]
    fn the_flavour_with_no_base_layer_is_singled_out_in_the_reasons() {
        let profile = ClientProfile::conservative_browser();
        let source = source(
            "matroska,webm",
            vec![
                video_track(
                    0,
                    "h264",
                    2160,
                    Some(HdrFormat::DolbyVision { profile: Some(5) }),
                ),
                audio_track(1, "aac", 2, true),
            ],
        );
        let decision = decide(&source, &request(&profile));

        assert!(decision.reasons.iter().any(|reason| matches!(
            reason,
            Reason::DolbyVisionWithoutBaseLayer { profile: Some(5) }
        )));
    }

    /// A client that opens the container and shows wide gamut on the given
    /// codec and curves.
    fn showing_wide_gamut(codec: &str, curves: &[Curve]) -> ClientProfile {
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        profile.video.push(VideoCapability::any("hevc"));
        profile.wide_gamut = curves
            .iter()
            .map(|curve| WideGamutCapability {
                codec: codec.into(),
                curve: *curve,
            })
            .collect();
        profile
    }

    fn wide_gamut_film(codec: &str, format: HdrFormat) -> MediaSource {
        source(
            "matroska,webm",
            vec![
                video_track(0, codec, 2160, Some(format)),
                audio_track(1, "aac", 2, true),
            ],
        )
    }

    #[test]
    fn a_client_that_shows_wide_gamut_gets_the_stream_untouched() {
        let profile = showing_wide_gamut("hevc", &[Curve::Pq]);
        let decision = decide(&wide_gamut_film("hevc", HdrFormat::Hdr10), &request(&profile));

        assert_eq!(decision.method, PlaybackMethod::DirectPlay);
        assert!(!decision.tone_map);
    }

    #[test]
    fn wide_gamut_shown_for_another_codec_or_curve_is_still_converted() {
        let profile = showing_wide_gamut("av1", &[Curve::Pq]);
        let decision = decide(&wide_gamut_film("hevc", HdrFormat::Hdr10), &request(&profile));
        assert!(decision.tone_map, "shown for another codec only");

        let profile = showing_wide_gamut("hevc", &[Curve::Pq]);
        let decision = decide(&wide_gamut_film("hevc", HdrFormat::Hlg), &request(&profile));
        assert!(decision.tone_map, "shown on another curve only");
        assert!(decision
            .reasons
            .contains(&Reason::WideGamutNotSupported { format: HdrFormat::Hlg }));
    }

    #[test]
    fn the_flavour_with_no_base_layer_is_converted_even_for_a_screen_that_shows_wide_gamut() {
        // What the old rule only avoided by taking every screen for one of
        // standard range: handed over as it is, this is green and purple.
        let profile = showing_wide_gamut("hevc", &[Curve::Pq, Curve::Hlg]);
        let decision = decide(
            &wide_gamut_film("hevc", HdrFormat::DolbyVision { profile: Some(5) }),
            &PlaybackRequest {
                wide_gamut: WideGamutChoice::NeverConvert,
                ..request(&profile)
            },
        );
        assert!(decision.tone_map);
        assert_eq!(decision.method, PlaybackMethod::FullTranscode);
        assert!(!decision.wide_gamut_follows_choice, "imposed, whatever was chosen");
    }

    #[test]
    fn a_picture_rebuilt_for_another_reason_comes_out_converted() {
        // Shown as it is, but asked smaller: a rebuilt picture is of standard
        // range, so its colours are converted on the way or come out wrong.
        let mut profile = showing_wide_gamut("hevc", &[Curve::Pq]);
        profile.max_height = Some(1080);
        let decision = decide(
            &wide_gamut_film("hevc", HdrFormat::Hdr10),
            &PlaybackRequest {
                wide_gamut: WideGamutChoice::NeverConvert,
                ..request(&profile)
            },
        );
        assert!(decision.tone_map);
        assert!(decision
            .reasons
            .contains(&Reason::WideGamutRebuiltAsStandard { format: HdrFormat::Hdr10 }));
        assert!(!decision.wide_gamut_follows_choice);
    }

    #[test]
    fn the_viewer_can_ask_for_the_conversion_or_to_keep_the_colour() {
        let shows = showing_wide_gamut("hevc", &[Curve::Pq]);
        let asked = decide(
            &wide_gamut_film("hevc", HdrFormat::Hdr10),
            &PlaybackRequest {
                wide_gamut: WideGamutChoice::AlwaysConvert,
                ..request(&shows)
            },
        );
        assert!(asked.tone_map, "converted though the screen was said to show it");
        assert!(asked
            .reasons
            .contains(&Reason::WideGamutConversionChosen { format: HdrFormat::Hdr10 }));

        let mut shows_nothing = ClientProfile::conservative_browser();
        shows_nothing.containers.push("matroska".into());
        shows_nothing.video.push(VideoCapability::any("hevc"));
        let kept = decide(
            &wide_gamut_film("hevc", HdrFormat::Hdr10),
            &PlaybackRequest {
                wide_gamut: WideGamutChoice::NeverConvert,
                ..request(&shows_nothing)
            },
        );
        assert!(!kept.tone_map, "kept though the screen was not said to show it");
        assert_eq!(kept.method, PlaybackMethod::DirectPlay);
        assert!(kept.wide_gamut_follows_choice);
        assert!(asked.wide_gamut_follows_choice);
    }

    #[test]
    fn asking_for_a_fold_rebuilds_audio_the_client_could_have_played_as_is() {
        // The point of making the preference an input of the decision: without
        // it, the setting would quietly do nothing here.
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        profile.audio_codecs.push("aac".into());
        profile.max_audio_channels = Some(8);

        let source = source(
            "matroska,webm",
            vec![
                video_track(0, "h264", 1080, None),
                audio_track(1, "aac", 6, true),
            ],
        );

        let untouched = decide(&source, &request(&profile));
        assert_eq!(untouched.method, PlaybackMethod::DirectPlay);

        let folded = decide(
            &source,
            &PlaybackRequest {
                downmix: DownmixMethod::NightDialogue,
                ..request(&profile)
            },
        );
        assert_eq!(folded.method, PlaybackMethod::TranscodeAudio);
        assert!(folded.reasons.iter().any(|reason| matches!(
            reason,
            Reason::DownmixRequested {
                method: DownmixMethod::NightDialogue
            }
        )));
    }

    #[test]
    fn a_fold_changes_nothing_on_a_track_that_is_already_stereo() {
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        let source = source(
            "matroska,webm",
            vec![
                video_track(0, "h264", 1080, None),
                audio_track(1, "aac", 2, true),
            ],
        );
        let decision = decide(
            &source,
            &PlaybackRequest {
                downmix: DownmixMethod::BroadcastStandard,
                ..request(&profile)
            },
        );
        assert_eq!(decision.method, PlaybackMethod::DirectPlay);
    }

    #[test]
    fn picking_another_audio_track_forces_a_rebuild_because_browsers_cannot_switch() {
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        let tracks = vec![
            video_track(0, "h264", 1080, None),
            audio_track(1, "aac", 2, true),
            audio_track(2, "aac", 2, false),
        ];
        let source = source("matroska,webm", tracks);
        let second = source.tracks[2].clone();

        let decision = decide(
            &source,
            &PlaybackRequest {
                audio_track: Some(&second),
                ..request(&profile)
            },
        );

        assert_eq!(decision.method, PlaybackMethod::Remux);
        assert_eq!(decision.audio_stream_index, Some(2));
        assert!(decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::NonDefaultTrackSelected)));
    }

    #[test]
    fn picking_the_track_the_file_already_defaults_to_changes_nothing() {
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        let source = source(
            "matroska,webm",
            vec![
                video_track(0, "h264", 1080, None),
                audio_track(1, "aac", 2, true),
            ],
        );
        let default = source.tracks[1].clone();
        let decision = decide(
            &source,
            &PlaybackRequest {
                audio_track: Some(&default),
                ..request(&profile)
            },
        );
        assert_eq!(decision.method, PlaybackMethod::DirectPlay);
    }

    #[test]
    fn a_file_whose_default_track_is_not_its_first_still_needs_a_rebuild_to_show_it() {
        // The defect this exists for: a real file flagged its French track
        // the one to default to, sitting second in the file, and a real
        // browser handed that file whole played its English track anyway, the
        // one sitting first. Picking French therefore has to force a rebuild
        // exactly as picking any other non-first track would, whatever the
        // file itself says its default is: nothing here has ever proved a
        // browser reads that flag, and the one browser tested here does not.
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        let tracks = vec![
            video_track(0, "h264", 1080, None),
            audio_track(1, "aac", 2, false),
            audio_track(2, "aac", 2, true),
        ];
        let source = source("matroska,webm", tracks);
        let flagged_as_default = source.tracks[2].clone();

        let decision = decide(
            &source,
            &PlaybackRequest {
                audio_track: Some(&flagged_as_default),
                ..request(&profile)
            },
        );

        assert_eq!(decision.method, PlaybackMethod::Remux);
        assert_eq!(decision.audio_stream_index, Some(2));
        assert!(decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::NonDefaultTrackSelected)));
    }

    #[test]
    fn automatically_reaching_for_a_track_that_is_not_first_still_forces_a_rebuild() {
        // The same defect, without anybody picking anything: nobody asked for
        // a track, so the file's own default is what gets recommended, and
        // that recommendation is worth nothing if a plain hand-over would show
        // something else. A viewer who never touched the menu is entitled to
        // hear the same track the page says is playing.
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        let tracks = vec![
            video_track(0, "h264", 1080, None),
            audio_track(1, "aac", 2, false),
            audio_track(2, "aac", 2, true),
        ];
        let source = source("matroska,webm", tracks);

        let decision = decide(&source, &request(&profile));

        assert_eq!(
            decision.audio_stream_index,
            Some(2),
            "the file's own default is still what is recommended"
        );
        assert_eq!(
            decision.method,
            PlaybackMethod::Remux,
            "and a rebuild is what actually delivers it"
        );
    }

    #[test]
    fn text_subtitles_travel_beside_the_video_and_stay_switchable() {
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        let tracks = vec![
            video_track(0, "h264", 1080, None),
            audio_track(1, "aac", 2, true),
            subtitle_track(2, "subrip", SubtitleLayout::Text),
        ];
        let source = source("matroska,webm", tracks);
        let subtitle = source.tracks[2].clone();

        let decision = decide(
            &source,
            &PlaybackRequest {
                subtitle_track: Some(&subtitle),
                ..request(&profile)
            },
        );

        assert_eq!(decision.subtitles, SubtitleDelivery::External);
        assert_eq!(
            decision.video,
            StreamAction::Copy,
            "the picture is untouched"
        );
        assert_eq!(decision.method, PlaybackMethod::Remux);
        assert!(
            !decision.is_direct_play(),
            "a stream that had to be rebuilt in any way is not direct play"
        );
        assert!(
            !decision.reasons.contains(&Reason::EverythingSupported),
            "everything was not supported: the subtitle had to be sent apart"
        );
    }

    #[test]
    fn a_stream_rebuilt_only_to_carry_a_subtitle_is_not_called_untouched() {
        // A client that can switch tracks inside the container asks for a
        // subtitle and nothing else stands in the way: the answer is still a
        // rebuild, and saying everything was supported would be a lie.
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        profile.can_switch_tracks_in_container = true;
        let tracks = vec![
            video_track(0, "h264", 1080, None),
            audio_track(1, "aac", 2, true),
            subtitle_track(2, "subrip", SubtitleLayout::Text),
        ];
        let source = source("matroska,webm", tracks);
        let subtitle = source.tracks[2].clone();

        let decision = decide(
            &source,
            &PlaybackRequest {
                subtitle_track: Some(&subtitle),
                ..request(&profile)
            },
        );

        assert_eq!(decision.method, PlaybackMethod::Remux);
        assert!(!decision.reasons.contains(&Reason::EverythingSupported));
    }

    #[test]
    fn a_client_that_draws_no_subtitle_has_them_drawn_into_the_picture() {
        // Sending a track it cannot draw would leave a viewer who asked for
        // subtitles looking at none.
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        profile.subtitle_formats.clear();
        let tracks = vec![
            video_track(0, "h264", 1080, None),
            audio_track(1, "aac", 2, true),
            subtitle_track(2, "subrip", SubtitleLayout::Text),
        ];
        let source = source("matroska,webm", tracks);
        let subtitle = source.tracks[2].clone();

        let decision = decide(
            &source,
            &PlaybackRequest {
                subtitle_track: Some(&subtitle),
                ..request(&profile)
            },
        );

        assert_eq!(decision.subtitles, SubtitleDelivery::BurnIn);
        assert_eq!(decision.method, PlaybackMethod::FullTranscode);
        assert!(decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::SubtitleFormatNotDrawnByClient { .. })));
    }

    #[test]
    fn a_file_holding_no_picture_is_not_described_as_carrying_one() {
        // Not a film, but the same decision answers for every source, and an
        // answer that claims a picture is copied where there is none would
        // send the player looking for a stream that does not exist.
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        let source = source("matroska,webm", vec![audio_track(0, "aac", 2, true)]);
        let decision = decide(&source, &request(&profile));

        assert_eq!(decision.method, PlaybackMethod::DirectPlay);
        assert_eq!(decision.video, StreamAction::Drop);
        assert_eq!(decision.video_stream_index, None);
    }

    #[test]
    fn the_client_is_told_which_way_the_stream_is_produced() {
        // These words travel to the client and to the activity page, so they
        // are pinned rather than left to drift.
        assert_eq!(PlaybackMethod::DirectPlay.as_str(), "direct_play");
        assert_eq!(PlaybackMethod::Remux.as_str(), "remux");
        assert_eq!(PlaybackMethod::TranscodeAudio.as_str(), "transcode_audio");
        assert_eq!(PlaybackMethod::FullTranscode.as_str(), "full_transcode");

        // Only the two that decode anything count against the limit.
        assert!(!PlaybackMethod::DirectPlay.is_expensive());
        assert!(!PlaybackMethod::Remux.is_expensive());
        assert!(PlaybackMethod::TranscodeAudio.is_expensive());
        assert!(PlaybackMethod::FullTranscode.is_expensive());
    }

    #[test]
    fn picture_subtitles_have_to_be_drawn_into_the_image() {
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        let tracks = vec![
            video_track(0, "h264", 1080, None),
            audio_track(1, "aac", 2, true),
            subtitle_track(2, "hdmv_pgs_subtitle", SubtitleLayout::Bitmap),
        ];
        let source = source("matroska,webm", tracks);
        let subtitle = source.tracks[2].clone();

        let decision = decide(
            &source,
            &PlaybackRequest {
                subtitle_track: Some(&subtitle),
                ..request(&profile)
            },
        );

        assert_eq!(decision.subtitles, SubtitleDelivery::BurnIn);
        assert_eq!(decision.method, PlaybackMethod::FullTranscode);
        assert_eq!(decision.video, StreamAction::Transcode);
        assert!(decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::SubtitleMustBeBurnedIn { .. })));
    }

    #[test]
    fn a_picture_taller_than_the_client_accepts_is_scaled_down() {
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        profile.max_height = Some(1080);
        let source = source(
            "matroska,webm",
            vec![
                video_track(0, "h264", 2160, None),
                audio_track(1, "aac", 2, true),
            ],
        );
        let decision = decide(&source, &request(&profile));

        assert_eq!(decision.method, PlaybackMethod::FullTranscode);
        assert_eq!(decision.scale_to_height, Some(1080));
        assert!(decision.reasons.iter().any(|reason| matches!(
            reason,
            Reason::ResolutionTooHigh {
                height: 2160,
                max_height: 1080
            }
        )));
    }

    #[test]
    fn a_picture_that_arrives_faster_than_the_client_accepts_is_rebuilt() {
        // The case this guards against is a viewer on a thin connection: the
        // file plays for two seconds and then stalls for ever.
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        profile.max_bitrate = Some(8_000_000);

        let mut fast = video_track(0, "h264", 1080, None);
        if let TrackKind::Video(details) = &mut fast.kind {
            details.bitrate = Some(25_000_000);
        }
        let source = source("matroska,webm", vec![fast, audio_track(1, "aac", 2, true)]);
        let decision = decide(&source, &request(&profile));

        assert_eq!(decision.method, PlaybackMethod::FullTranscode);
        assert!(decision.reasons.iter().any(|reason| matches!(
            reason,
            Reason::BitrateTooHigh {
                bitrate: 25_000_000,
                max_bitrate: 8_000_000
            }
        )));
    }

    #[test]
    fn a_rate_asked_for_travels_with_the_answer_so_it_is_the_one_produced() {
        // Asking for a rate is what turned this into a rebuild. Producing it
        // at some other rate would be the cost of the work without the point
        // of it, and the viewer on the thin connection would stall anyway.
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        profile.max_bitrate = Some(4_000_000);

        let mut fast = video_track(0, "h264", 1080, None);
        if let TrackKind::Video(details) = &mut fast.kind {
            details.bitrate = Some(25_000_000);
        }
        let too_fast = source("matroska,webm", vec![fast, audio_track(1, "aac", 2, true)]);
        assert_eq!(
            decide(&too_fast, &request(&profile)).bitrate_ceiling,
            Some(4_000_000)
        );

        // Nothing rebuilt, so nothing to hold to a rate: the file arrives at
        // the rate it was written at whatever anybody asked for.
        let slow = source(
            "matroska,webm",
            vec![
                video_track(0, "h264", 1080, None),
                audio_track(1, "aac", 2, true),
            ],
        );
        let decision = decide(&slow, &request(&profile));
        assert_eq!(decision.method, PlaybackMethod::DirectPlay);
        assert_eq!(decision.bitrate_ceiling, None);
    }

    #[test]
    fn a_film_stating_no_rate_per_stream_is_judged_on_the_rate_of_the_whole_file() {
        // Most films in a collection state none, and a limit that quietly does
        // nothing on most films is worse than no limit at all: somebody sets
        // it, sees no change, and concludes the setting is broken.
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        profile.max_bitrate = Some(8_000_000);

        let mut source = source(
            "matroska,webm",
            vec![
                video_track(0, "h264", 1080, None),
                audio_track(1, "aac", 2, true),
            ],
        );
        assert_eq!(
            decide(&source, &request(&profile)).method,
            PlaybackMethod::DirectPlay,
            "nothing says how fast this arrives, so nothing says it is too fast"
        );

        source.overall_bitrate = Some(30_000_000);
        let decision = decide(&source, &request(&profile));
        assert_eq!(decision.method, PlaybackMethod::FullTranscode);
        assert!(decision.reasons.iter().any(|reason| matches!(
            reason,
            Reason::BitrateTooHigh {
                bitrate: 30_000_000,
                ..
            }
        )));
    }

    #[test]
    fn a_picture_exactly_at_the_bitrate_the_client_accepts_is_left_alone() {
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        profile.max_bitrate = Some(8_000_000);

        let mut exactly = video_track(0, "h264", 1080, None);
        if let TrackKind::Video(details) = &mut exactly.kind {
            details.bitrate = Some(8_000_000);
        }
        let source = source(
            "matroska,webm",
            vec![exactly, audio_track(1, "aac", 2, true)],
        );
        let decision = decide(&source, &request(&profile));

        assert_eq!(
            decision.method,
            PlaybackMethod::DirectPlay,
            "the limit is what the client accepts, not what it refuses"
        );
        assert!(!decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::BitrateTooHigh { .. })));
    }

    #[test]
    fn a_picture_exactly_as_tall_as_the_client_accepts_is_left_alone() {
        // The boundary matters: rescaling a picture to the size it already is
        // costs a full rebuild and gives the viewer exactly what they had.
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        profile.max_height = Some(1080);
        let source = source(
            "matroska,webm",
            vec![
                video_track(0, "h264", 1080, None),
                audio_track(1, "aac", 2, true),
            ],
        );
        let decision = decide(&source, &request(&profile));

        assert_eq!(decision.scale_to_height, None);
        assert!(!decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::ResolutionTooHigh { .. })));
        assert_eq!(decision.method, PlaybackMethod::DirectPlay);
    }

    #[test]
    fn an_interlaced_picture_is_rebuilt_so_it_looks_right_on_a_screen() {
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        let mut tracks = vec![
            video_track(0, "h264", 576, None),
            audio_track(1, "aac", 2, true),
        ];
        if let TrackKind::Video(details) = &mut tracks[0].kind {
            details.is_interlaced = true;
        }
        let source = source("matroska,webm", tracks);
        let decision = decide(&source, &request(&profile));

        assert_eq!(decision.method, PlaybackMethod::FullTranscode);
        assert!(decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::InterlacedPicture)));
    }

    #[test]
    fn a_supported_codec_in_an_unsupported_profile_says_so_precisely() {
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());
        profile.video = vec![crate::profile::VideoCapability {
            codec: "h264".into(),
            profiles: vec!["Main".into()],
            max_level: None,
        }];
        let source = source(
            "matroska,webm",
            vec![
                video_track(0, "h264", 1080, None),
                audio_track(1, "aac", 2, true),
            ],
        );
        let decision = decide(&source, &request(&profile));

        assert!(decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::VideoProfileNotSupported { .. })));
        assert!(
            !decision
                .reasons
                .iter()
                .any(|reason| matches!(reason, Reason::VideoCodecNotSupported { .. })),
            "the codec is known, only the profile is not"
        );
    }

    #[test]
    fn the_open_container_and_the_web_one_are_told_apart_despite_sharing_a_family_name() {
        // The analyser reports the same family for both, and a browser reads
        // only one of them. Matching any member of the family would call an
        // unplayable file directly playable.
        let profile = ClientProfile::conservative_browser();
        let tracks = vec![
            video_track(0, "vp9", 1080, None),
            audio_track(1, "opus", 2, true),
        ];

        let web = source_named("clip.webm", "matroska,webm", tracks.clone());
        assert_eq!(
            decide(&web, &request(&profile)).method,
            PlaybackMethod::DirectPlay,
            "the web container is readable as it is"
        );

        let open = source_named("film.mkv", "matroska,webm", tracks);
        assert_eq!(
            decide(&open, &request(&profile)).method,
            PlaybackMethod::Remux,
            "the open container has to be repackaged"
        );
    }

    #[test]
    fn a_file_with_no_usable_extension_falls_back_to_the_reported_name() {
        let source = source_named(
            "film",
            "matroska,webm",
            vec![video_track(0, "h264", 1080, None)],
        );
        assert_eq!(effective_container(&source).as_deref(), Some("matroska"));
    }

    #[test]
    fn the_extension_settles_the_container_whatever_the_analyser_reports() {
        let tracks = vec![video_track(0, "h264", 1080, None)];
        for (name, expected) in [
            ("film.mkv", "matroska"),
            ("clip.webm", "webm"),
            ("film.mp4", "mp4"),
            ("film.MP4", "mp4"),
            ("film.mov", "mov"),
        ] {
            let source = source_named(name, "whatever,the,analyser,said", tracks.clone());
            assert_eq!(
                effective_container(&source).as_deref(),
                Some(expected),
                "for {name}"
            );
        }
    }

    #[test]
    fn a_decision_always_carries_at_least_one_reason() {
        let profile = ClientProfile::conservative_browser();
        for container in ["mov,mp4,m4a", "matroska,webm"] {
            for audio in ["aac", "eac3"] {
                let source = source(
                    container,
                    vec![
                        video_track(0, "h264", 1080, None),
                        audio_track(1, audio, 2, true),
                    ],
                );
                let decision = decide(&source, &request(&profile));
                assert!(
                    !decision.reasons.is_empty(),
                    "every answer must be explainable"
                );
            }
        }
    }

    #[test]
    fn a_source_with_no_audio_track_is_handled_rather_than_assumed_away() {
        let profile = ClientProfile::conservative_browser();
        let source = source("mov,mp4,m4a", vec![video_track(0, "h264", 1080, None)]);
        let decision = decide(&source, &request(&profile));

        assert_eq!(decision.audio, StreamAction::Drop);
        assert_eq!(decision.audio_stream_index, None);
        assert_eq!(decision.method, PlaybackMethod::DirectPlay);
    }

    #[test]
    fn an_audio_description_is_passed_over_even_when_the_file_calls_it_its_default() {
        let profile = ClientProfile::conservative_browser();
        let mut described = audio_track(1, "eac3", 2, true);
        described.title = Some("Audio Description".into());
        let ordinary = audio_track(2, "aac", 2, false);
        let source = source(
            "matroska,webm",
            vec![video_track(0, "h264", 1080, None), described, ordinary],
        );

        let decision = decide(&source, &request(&profile));
        assert_eq!(decision.audio_stream_index, Some(2));
    }

    #[test]
    fn an_audio_description_still_plays_when_it_is_the_only_sound_there_is() {
        let profile = ClientProfile::conservative_browser();
        let mut described = audio_track(1, "aac", 2, true);
        described.title = Some("Audio Description".into());
        let source = source(
            "matroska,webm",
            vec![video_track(0, "h264", 1080, None), described],
        );

        let decision = decide(&source, &request(&profile));
        assert_eq!(decision.audio_stream_index, Some(1));
    }

    #[test]
    fn levelling_loudness_only_rebuilds_audio_when_a_measurement_exists() {
        let mut profile = ClientProfile::conservative_browser();
        profile.containers.push("matroska".into());

        let mut tracks = vec![
            video_track(0, "h264", 1080, None),
            audio_track(1, "aac", 2, true),
        ];
        let unmeasured = source("matroska,webm", tracks.clone());
        let decision = decide(
            &unmeasured,
            &PlaybackRequest {
                level_loudness: true,
                ..request(&profile)
            },
        );
        assert_eq!(
            decision.method,
            PlaybackMethod::DirectPlay,
            "nothing to level with, so nothing is rebuilt"
        );

        if let TrackKind::Audio(details) = &mut tracks[1].kind {
            details.loudness = Loudness {
                integrated_lufs: Some(-18.0),
                true_peak_dbfs: Some(-1.0),
                range_lu: Some(7.0),
            };
        }
        let measured = source("matroska,webm", tracks);
        let decision = decide(
            &measured,
            &PlaybackRequest {
                level_loudness: true,
                ..request(&profile)
            },
        );
        assert_eq!(decision.method, PlaybackMethod::TranscodeAudio);
        assert!(decision
            .reasons
            .iter()
            .any(|reason| matches!(reason, Reason::LoudnessLevellingRequested)));
    }
}
