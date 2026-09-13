//! Building a media tool invocation as a value.
//!
//! A command is a structure, and turning it into arguments happens once, at
//! the end. Nothing here ever concatenates a string. That buys three things:
//! the order of options stops being a source of bugs, a file name beginning
//! with a dash cannot be read as an option, and every decision can be checked
//! by a test without running anything.
//!
//! The video side is deliberately split into three blocks, decoding, filtering
//! and encoding, because that is exactly where a hardware path differs from a
//! software one. Keeping them apart now is what lets a card be plugged in
//! later without turning the builder into a thicket of conditions.

use std::ffi::OsString;
use std::path::PathBuf;

use melyxar_core::time::Millis;
use melyxar_core::user::DownmixMethod;

/// The input file and where to start reading it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    pub path: PathBuf,
    /// Where to start. Placed before the input so the tool seeks rather than
    /// decoding and discarding everything up to that point, which is the
    /// difference between an instant start and a long wait.
    pub start_at: Option<Millis>,
}

impl Input {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            start_at: None,
        }
    }

    pub fn starting_at(mut self, position: Millis) -> Self {
        self.start_at = Some(position);
        self
    }
}

/// Which streams of the input to carry over.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StreamSelection {
    pub video_index: Option<i32>,
    pub audio_index: Option<i32>,
    pub subtitle_index: Option<i32>,
}

/// What to do with the video.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoOutput {
    /// Drop it entirely.
    None,
    /// Carry the stream over untouched. No decoding, no re-encoding, which is
    /// what makes remuxing nearly free.
    Copy,
    /// Decode and encode again.
    Encode(VideoEncode),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoEncode {
    /// Encoder name, as the tool knows it.
    pub encoder: String,
    /// Quality setting, lower meaning better for the software encoder.
    pub quality: Option<u8>,
    /// Speed preset, trading time against size.
    pub preset: Option<String>,
    /// Cap on the produced bitrate, in bits per second.
    pub max_bitrate: Option<i64>,
    /// Height to scale down to, keeping the aspect ratio.
    pub scale_to_height: Option<i32>,
    /// Convert a wide gamut picture to standard range.
    ///
    /// Not an option in practice: no browser shows wide gamut correctly today,
    /// so a picture left unconverted comes out washed out and grey.
    pub tone_map: bool,
    /// Force a key frame at this interval, in milliseconds.
    ///
    /// This is what makes segments land on exact boundaries, which in turn is
    /// what lets the server own the playlist and hand out any segment on
    /// demand instead of waiting for the encoder to reach it.
    pub keyframe_interval: Option<Millis>,
}

impl VideoEncode {
    /// Software encoding to the codec every browser reads.
    pub fn software_h264() -> Self {
        Self {
            encoder: "libx264".to_string(),
            quality: Some(23),
            preset: Some("veryfast".to_string()),
            max_bitrate: None,
            scale_to_height: None,
            tone_map: false,
            keyframe_interval: None,
        }
    }
}

/// What to do with the audio.
#[derive(Debug, Clone, PartialEq)]
pub enum AudioOutput {
    None,
    Copy,
    Encode(AudioEncode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioEncode {
    pub encoder: String,
    pub bitrate: Option<i64>,
    /// Number of output channels. Two for a browser.
    pub channels: Option<i32>,
    /// How multichannel audio is folded down.
    pub downmix: DownmixMethod,
    /// Gain applied after folding, because folding lowers the perceived level.
    pub downmix_gain: f64,
    /// Gain applied to even out loudness between files, from the measurement
    /// taken during the background pass.
    pub loudness_gain_db: Option<f64>,
    /// Catch peaks after the gain instead of letting them clip.
    ///
    /// Insurance rather than a fix: film soundtracks carry enough headroom
    /// that a gain of two or three rarely clips. It matters on material that
    /// was already mastered loud. Set high enough to stay inaudible, because a
    /// limiter that keeps engaging sounds worse than the problem it prevents.
    pub limiter: bool,
}

impl AudioEncode {
    /// Stereo output for a browser, with the balanced fold and its gain.
    pub fn browser_stereo(encoder: impl Into<String>) -> Self {
        Self {
            encoder: encoder.into(),
            bitrate: Some(192_000),
            channels: Some(2),
            downmix: DownmixMethod::BroadcastStandard,
            downmix_gain: melyxar_core::user::DEFAULT_DOWNMIX_GAIN,
            loudness_gain_db: None,
            limiter: true,
        }
    }
}

/// What the command produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    /// A plain file.
    File(PathBuf),
    /// Numbered segments plus the header they share, for streaming.
    ///
    /// The tool only ever produces files here. The playlist a player reads is
    /// written by the server, which knows the whole duration from the analysis
    /// and can therefore hand out any segment on demand. The tool writes one
    /// of its own as a byproduct, which is never served.
    Segments {
        /// Pattern the numbered files follow.
        pattern: PathBuf,
        /// The header every segment needs, written once beside them.
        initialisation: PathBuf,
        /// Where the tool writes its own playlist. A byproduct: the server
        /// writes the one a player reads.
        tool_playlist: PathBuf,
        /// Nominal length of one segment.
        duration: Millis,
        /// Number the first produced segment carries, so that a jump starts
        /// the encoder further in rather than from the beginning.
        start_number: u32,
    },
    /// A single image, for a thumbnail.
    StillImage(PathBuf),
    /// Nothing kept, used when only the measurement matters.
    Discard,
}

/// A full invocation.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub input: Input,
    pub streams: StreamSelection,
    pub video: VideoOutput,
    pub audio: AudioOutput,
    /// Stop after this much of the input.
    pub duration: Option<Millis>,
    /// Report progress on standard output, in a form meant for programs.
    ///
    /// Parsing the human-facing status line instead is the usual shortcut and
    /// it breaks whenever the wording changes.
    pub report_progress: bool,
    /// Extra arguments, for a filter a hardware path needs that the structure
    /// does not model yet.
    pub extra_arguments: Vec<String>,
    pub output: Output,
}

impl Command {
    pub fn new(input: Input, output: Output) -> Self {
        Self {
            input,
            streams: StreamSelection::default(),
            video: VideoOutput::Copy,
            audio: AudioOutput::Copy,
            duration: None,
            report_progress: false,
            extra_arguments: Vec::new(),
            output,
        }
    }

    pub fn with_streams(mut self, streams: StreamSelection) -> Self {
        self.streams = streams;
        self
    }

    pub fn with_video(mut self, video: VideoOutput) -> Self {
        self.video = video;
        self
    }

    pub fn with_audio(mut self, audio: AudioOutput) -> Self {
        self.audio = audio;
        self
    }

    pub fn reporting_progress(mut self) -> Self {
        self.report_progress = true;
        self
    }

    /// Whether this command re-encodes anything.
    ///
    /// Copying both streams costs almost nothing, so sessions that only remux
    /// are not counted against the transcoding limit.
    pub fn is_transcoding(&self) -> bool {
        matches!(self.video, VideoOutput::Encode(_)) || matches!(self.audio, AudioOutput::Encode(_))
    }

    /// Turns the command into the argument list to hand to the tool.
    pub fn to_arguments(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = Vec::new();
        // A macro rather than a closure: the output branches also push paths
        // directly, and a closure holding a mutable borrow would forbid that.
        macro_rules! push {
            ($value:expr) => {
                args.push(OsString::from($value))
            };
        }

        push!("-hide_banner");
        push!("-nostdin");
        push!("-loglevel");
        push!("error");

        if self.report_progress {
            push!("-progress");
            push!("pipe:1");
            push!("-nostats");
        }

        // Seeking before the input makes the tool jump there; after the input
        // it would decode and throw away everything up to that point.
        if let Some(start) = self.input.start_at {
            push!("-ss");
            push!(&format_seconds(start));
        }

        push!("-i");
        // Pushed as a path rather than a string: a name starting with a dash
        // must never be read as an option.
        args.push(self.input.path.clone().into_os_string());

        if let Some(duration) = self.duration {
            push!("-t");
            push!(&format_seconds(duration));
        }

        // The segment muxer does not apply the tool's automatic stream
        // selection: without an explicit mapping it produces an output with no
        // stream at all and refuses to start. Every other output form is happy
        // to choose on its own.
        let needs_explicit_mapping = matches!(self.output, Output::Segments { .. });

        match (self.streams.video_index, &self.video) {
            (Some(index), _) => {
                push!("-map");
                push!(&format!("0:{index}"));
            }
            (None, VideoOutput::None) => push!("-vn"),
            // The trailing marker makes the mapping optional, so a file
            // carrying no such stream is served rather than refused.
            (None, _) if needs_explicit_mapping => {
                push!("-map");
                push!("0:v:0?");
            }
            (None, _) => {}
        }

        match (self.streams.audio_index, &self.audio) {
            (Some(index), _) => {
                push!("-map");
                push!(&format!("0:{index}"));
            }
            (None, AudioOutput::None) => push!("-an"),
            (None, _) if needs_explicit_mapping => {
                push!("-map");
                push!("0:a:0?");
            }
            (None, _) => {}
        }

        if let Some(index) = self.streams.subtitle_index {
            push!("-map");
            push!(&format!("0:{index}"));
        }

        match &self.video {
            VideoOutput::None => {}
            VideoOutput::Copy => {
                push!("-c:v");
                push!("copy");
            }
            VideoOutput::Encode(encode) => {
                let mut filters: Vec<String> = Vec::new();
                // Conversion comes before scaling: mapping colours on the
                // full size picture then shrinking gives a cleaner result
                // than the other way round.
                if encode.tone_map {
                    filters.push(TONE_MAP_FILTER.to_string());
                }
                if let Some(height) = encode.scale_to_height {
                    filters.push(format!("scale=-2:{height}"));
                }
                if !filters.is_empty() {
                    push!("-vf");
                    push!(&filters.join(","));
                }

                push!("-c:v");
                push!(&encode.encoder);
                if let Some(preset) = &encode.preset {
                    push!("-preset");
                    push!(preset);
                }
                if let Some(quality) = encode.quality {
                    push!("-crf");
                    push!(&quality.to_string());
                }
                if let Some(bitrate) = encode.max_bitrate {
                    push!("-maxrate");
                    push!(&bitrate.to_string());
                    // A buffer twice the cap is the usual pairing: smaller
                    // starves the encoder, larger lets the rate wander.
                    push!("-bufsize");
                    push!(&(bitrate * 2).to_string());
                }
                if let Some(interval) = encode.keyframe_interval {
                    // Forcing key frames on a fixed grid is what makes
                    // segments land on exact boundaries.
                    push!("-force_key_frames");
                    push!(&format!(
                        "expr:gte(t,n_forced*{})",
                        format_seconds(interval)
                    ));
                }
                // Wide compatibility beats a marginally smaller file here.
                push!("-pix_fmt");
                push!("yuv420p");
            }
        }

        match &self.audio {
            AudioOutput::None => {}
            AudioOutput::Copy => {
                push!("-c:a");
                push!("copy");
            }
            AudioOutput::Encode(encode) => {
                if let Some(filter) = audio_filter_chain(encode) {
                    push!("-af");
                    push!(&filter);
                }
                push!("-c:a");
                push!(&encode.encoder);
                if let Some(channels) = encode.channels {
                    push!("-ac");
                    push!(&channels.to_string());
                }
                if let Some(bitrate) = encode.bitrate {
                    push!("-b:a");
                    push!(&bitrate.to_string());
                }
            }
        }

        for extra in &self.extra_arguments {
            push!(extra);
        }

        match &self.output {
            Output::File(path) => {
                push!("-y");
                args.push(path.clone().into_os_string());
            }
            Output::Segments {
                pattern,
                initialisation,
                tool_playlist,
                duration,
                start_number,
            } => {
                push!("-f");
                push!("hls");
                push!("-hls_time");
                push!(&format_seconds(*duration));
                // The whole film, start to finish: nothing is dropped from the
                // list as it goes, which is what a viewer jumping backwards
                // would otherwise fall off the end of.
                push!("-hls_playlist_type");
                push!("vod");
                push!("-hls_list_size");
                push!("0");
                // Fragmented segments, which is what a browser can append to a
                // running stream and what the modern codecs require.
                push!("-hls_segment_type");
                push!("fmp4");
                push!("-hls_fmp4_init_filename");
                args.push(file_name_of(initialisation));
                push!("-hls_segment_filename");
                args.push(pattern.clone().into_os_string());
                push!("-start_number");
                push!(&start_number.to_string());
                push!("-y");
                args.push(tool_playlist.clone().into_os_string());
            }
            Output::StillImage(path) => {
                push!("-frames:v");
                push!("1");
                push!("-y");
                args.push(path.clone().into_os_string());
            }
            Output::Discard => {
                push!("-f");
                push!("null");
                push!("-");
            }
        }

        args
    }
}

/// Filter that maps a wide gamut picture into standard range.
///
/// Goes through a linear light stage rather than converting directly, which is
/// what keeps skin tones from turning grey. Used for streams and for every
/// still image pulled out of a file, because an unconverted frame is exactly
/// the washed out thumbnail seen on other servers.
const TONE_MAP_FILTER: &str = concat!(
    "zscale=transfer=linear:npl=100,",
    "format=gbrpf32le,",
    "zscale=primaries=bt709,",
    "tonemap=tonemap=hable:desat=0,",
    "zscale=transfer=bt709:matrix=bt709:range=limited,",
    "format=yuv420p"
);

/// Builds the audio filter chain: loudness levelling, fold to stereo, gain,
/// then the safety limiter, in that order.
///
/// Order matters. Folding before the gain means the gain compensates a known
/// loss; limiting last means it catches whatever the gain produced.
fn audio_filter_chain(encode: &AudioEncode) -> Option<String> {
    let mut stages: Vec<String> = Vec::new();

    if let Some(gain) = encode.loudness_gain_db {
        if gain.abs() > 0.01 {
            stages.push(format!("volume={gain:.2}dB"));
        }
    }

    if let Some(matrix) = downmix_matrix(encode.downmix, encode.channels) {
        stages.push(matrix);
    }

    if (encode.downmix_gain - 1.0).abs() > 0.01 && encode.downmix != DownmixMethod::None {
        stages.push(format!("volume={:.2}", encode.downmix_gain));
    }

    if encode.limiter && !stages.is_empty() {
        // Threshold just below full scale, with a long release so it never
        // becomes audible as pumping.
        stages.push("alimiter=limit=0.95:attack=5:release=50".to_string());
    }

    (!stages.is_empty()).then(|| stages.join(","))
}

/// Coefficients folding a six channel layout into stereo.
///
/// Each method is a named set of gains rather than a string copied around, so
/// the choice is one value in the model and one place in the builder.
fn downmix_matrix(method: DownmixMethod, channels: Option<i32>) -> Option<String> {
    // Folding only applies when going down to two channels.
    if channels != Some(2) {
        return None;
    }
    let coefficients = match method {
        // Leave it to the tool's own default.
        DownmixMethod::None => return None,
        // Centre and low frequency split into both sides. Keeps the level up,
        // at the cost of heavy bass and quiet dialogue.
        DownmixMethod::CentreAndBassSplit => {
            "FL=0.5*FC+0.707*FL+0.707*BL+0.5*LFE|FR=0.5*FC+0.707*FR+0.707*BR+0.5*LFE"
        }
        // Centre strongly favoured, everything else pulled back. Made for
        // watching late.
        DownmixMethod::NightDialogue => {
            "FL=0.8*FC+0.35*FL+0.18*BL+0.07*LFE|FR=0.8*FC+0.35*FR+0.18*BR+0.07*LFE"
        }
        // Surround spread across both sides while preserving intensity.
        DownmixMethod::IntensityPreserving => {
            "FL=0.374*FC+0.529*FL+0.320*BL+0.213*BR+0.374*LFE|\
             FR=0.374*FC+0.529*FR+0.320*BR+0.213*BL+0.374*LFE"
        }
        // Broadcast standard: measured attenuation on centre and surround,
        // low frequency channel dropped. The balanced default.
        DownmixMethod::BroadcastStandard => {
            "FL=1.0*FL+0.707*FC+0.707*BL|FR=1.0*FR+0.707*FC+0.707*BR"
        }
    };
    Some(format!("pan=stereo|{coefficients}"))
}

/// The last part of a path, which is what the tool wants for the shared
/// header: it writes it beside the playlist, and a full path there would be
/// taken for a name rather than a place.
fn file_name_of(path: &std::path::Path) -> OsString {
    path.file_name()
        .map(std::ffi::OsStr::to_os_string)
        .unwrap_or_else(|| path.to_path_buf().into_os_string())
}

/// Renders a position the way the tool expects it, in seconds.
fn format_seconds(value: Millis) -> String {
    format!("{:.3}", value.as_seconds_f64())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(command: &Command) -> Vec<String> {
        command
            .to_arguments()
            .into_iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect()
    }

    fn position(args: &[String], needle: &str) -> Option<usize> {
        args.iter().position(|value| value == needle)
    }

    #[test]
    fn a_copy_command_re_encodes_nothing() {
        let command = Command::new(
            Input::new("/media/film.mkv"),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        );
        assert!(!command.is_transcoding());
        let args = arguments(&command);
        assert!(args.contains(&"copy".to_string()));
        assert!(!args.contains(&"libx264".to_string()));
    }

    #[test]
    fn encoding_either_stream_counts_as_transcoding() {
        let base = Command::new(
            Input::new("/media/film.mkv"),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        );
        assert!(base
            .clone()
            .with_audio(AudioOutput::Encode(AudioEncode::browser_stereo("aac")))
            .is_transcoding());
        assert!(base
            .with_video(VideoOutput::Encode(VideoEncode::software_h264()))
            .is_transcoding());
    }

    #[test]
    fn the_start_position_goes_before_the_input_so_the_tool_seeks() {
        let command = Command::new(
            Input::new("/media/film.mkv").starting_at(Millis::new(65_000)),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        );
        let args = arguments(&command);
        let seek = position(&args, "-ss").expect("a start position is present");
        let input = position(&args, "-i").expect("an input is present");
        assert!(
            seek < input,
            "seeking after the input would decode everything up to that point"
        );
        assert!(args.contains(&"65.000".to_string()));
    }

    #[test]
    fn the_input_path_is_passed_as_a_path_so_a_leading_dash_is_never_an_option() {
        let command = Command::new(
            Input::new("/media/-strange-name.mkv"),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        );
        let args = command.to_arguments();
        let input = args
            .iter()
            .position(|value| value == "-i")
            .expect("an input is present");
        assert_eq!(args[input + 1], OsString::from("/media/-strange-name.mkv"));
    }

    #[test]
    fn conversion_to_standard_range_comes_before_scaling() {
        let mut encode = VideoEncode::software_h264();
        encode.tone_map = true;
        encode.scale_to_height = Some(1080);
        let command = Command::new(
            Input::new("/media/film.mkv"),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        )
        .with_video(VideoOutput::Encode(encode));

        let args = arguments(&command);
        let filters = args
            .iter()
            .find(|value| value.contains("tonemap"))
            .expect("a filter chain is present");
        let tone_map = filters.find("tonemap").expect("conversion is present");
        let scale = filters.find("scale=-2:").expect("scaling is present");
        assert!(
            tone_map < scale,
            "mapping colours before shrinking keeps a cleaner picture"
        );
    }

    #[test]
    fn a_forced_key_frame_interval_is_expressed_in_seconds() {
        let mut encode = VideoEncode::software_h264();
        encode.keyframe_interval = Some(Millis::new(4000));
        let command = Command::new(
            Input::new("/media/film.mkv"),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        )
        .with_video(VideoOutput::Encode(encode));

        let args = arguments(&command);
        assert!(args.iter().any(|value| value.contains("n_forced*4.000")));
    }

    #[test]
    fn the_balanced_fold_drops_the_low_frequency_channel() {
        let encode = AudioEncode::browser_stereo("aac");
        let chain = audio_filter_chain(&encode).expect("a chain is produced");
        assert!(chain.contains("pan=stereo"));
        assert!(
            !chain.contains("LFE"),
            "the balanced method drops the low frequency channel, which is what keeps a gain of three from clipping"
        );
    }

    #[test]
    fn the_night_method_favours_the_centre_channel_above_all_others() {
        let encode = AudioEncode {
            downmix: DownmixMethod::NightDialogue,
            ..AudioEncode::browser_stereo("aac")
        };
        let chain = audio_filter_chain(&encode).expect("a chain is produced");
        assert!(chain.contains("0.8*FC"), "dialogue must dominate: {chain}");
    }

    #[test]
    fn asking_for_no_fold_produces_no_fold_and_no_compensation_gain() {
        let encode = AudioEncode {
            downmix: DownmixMethod::None,
            downmix_gain: 3.0,
            loudness_gain_db: None,
            limiter: true,
            ..AudioEncode::browser_stereo("aac")
        };
        assert!(
            audio_filter_chain(&encode).is_none(),
            "without a fold there is nothing to compensate for"
        );
    }

    #[test]
    fn the_compensation_gain_is_applied_after_the_fold_and_the_limiter_last() {
        let encode = AudioEncode {
            downmix_gain: 3.0,
            ..AudioEncode::browser_stereo("aac")
        };
        let chain = audio_filter_chain(&encode).expect("a chain is produced");
        let fold = chain.find("pan=stereo").expect("a fold is present");
        let gain = chain.find("volume=3.00").expect("a gain is present");
        let limiter = chain.find("alimiter").expect("a limiter is present");
        assert!(
            fold < gain,
            "the gain compensates a loss, so it comes after"
        );
        assert!(gain < limiter, "the limiter catches what the gain produced");
    }

    #[test]
    fn loudness_levelling_comes_first_so_it_applies_to_the_original_material() {
        let encode = AudioEncode {
            loudness_gain_db: Some(-3.5),
            ..AudioEncode::browser_stereo("aac")
        };
        let chain = audio_filter_chain(&encode).expect("a chain is produced");
        let levelling = chain.find("volume=-3.50dB").expect("levelling is present");
        let fold = chain.find("pan=stereo").expect("a fold is present");
        assert!(levelling < fold);
    }

    #[test]
    fn folding_is_skipped_when_the_output_is_not_stereo() {
        let encode = AudioEncode {
            channels: Some(6),
            ..AudioEncode::browser_stereo("aac")
        };
        assert!(downmix_matrix(encode.downmix, encode.channels).is_none());
    }

    #[test]
    fn segment_output_starts_at_the_requested_number_so_a_jump_does_not_restart_from_zero() {
        let command = Command::new(
            Input::new("/media/film.mkv").starting_at(Millis::new(1_248_000)),
            Output::Segments {
                pattern: PathBuf::from("/tmp/session/segment-%05d.m4s"),
                initialisation: PathBuf::from("/tmp/session/init.mp4"),
                tool_playlist: PathBuf::from("/tmp/session/tool.m3u8"),
                duration: Millis::new(4000),
                start_number: 312,
            },
        );
        let args = arguments(&command);
        let index = position(&args, "-start_number").expect("a start number is present");
        assert_eq!(args[index + 1], "312");
    }

    #[test]
    fn segment_output_always_maps_its_streams_explicitly() {
        // The segment muxer does not apply automatic stream selection: an
        // unmapped output is refused outright, which cost a debugging session
        // the first time it happened.
        let command = Command::new(
            Input::new("/media/film.mkv"),
            Output::Segments {
                pattern: PathBuf::from("/tmp/session/segment-%05d.m4s"),
                initialisation: PathBuf::from("/tmp/session/init.mp4"),
                tool_playlist: PathBuf::from("/tmp/session/tool.m3u8"),
                duration: Millis::new(4000),
                start_number: 0,
            },
        );
        let args = arguments(&command);
        assert!(args.contains(&"0:v:0?".to_string()), "video must be mapped");
        assert!(args.contains(&"0:a:0?".to_string()), "audio must be mapped");
    }

    #[test]
    fn a_plain_file_output_leaves_stream_selection_to_the_tool() {
        let command = Command::new(
            Input::new("/media/film.mkv"),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        );
        let args = arguments(&command);
        assert!(
            !args.iter().any(|value| value.starts_with("0:v:")),
            "only the segment muxer needs an explicit mapping"
        );
    }

    #[test]
    fn progress_is_reported_in_a_form_meant_for_programs() {
        let command = Command::new(
            Input::new("/media/film.mkv"),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        )
        .reporting_progress();
        let args = arguments(&command);
        let index = position(&args, "-progress").expect("progress is requested");
        assert_eq!(args[index + 1], "pipe:1");
    }

    #[test]
    fn selecting_streams_maps_them_explicitly() {
        let command = Command::new(
            Input::new("/media/film.mkv"),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        )
        .with_streams(StreamSelection {
            video_index: Some(0),
            audio_index: Some(2),
            subtitle_index: None,
        });
        let args = arguments(&command);
        assert!(args.contains(&"0:0".to_string()));
        assert!(args.contains(&"0:2".to_string()));
    }

    #[test]
    fn dropping_a_stream_is_expressed_rather_than_left_to_chance() {
        let command = Command::new(
            Input::new("/media/film.mkv"),
            Output::StillImage(PathBuf::from("/tmp/frame.jpg")),
        )
        .with_audio(AudioOutput::None);
        let args = arguments(&command);
        assert!(args.contains(&"-an".to_string()));
        assert!(args.contains(&"-frames:v".to_string()));
    }
}
