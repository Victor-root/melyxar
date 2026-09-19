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

use crate::hardware::Card;

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

/// Who rebuilds the picture, which is what decides the options the tool takes.
///
/// A software encoder and a card express the same idea in settings neither
/// accepts from the other: one is given a quality and a speed preset, the other
/// a rate. Keeping the two apart as values rather than as optional fields is
/// what stops the builder becoming a thicket of conditions, and it is what
/// makes a nonsensical pairing impossible to write down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rebuilding {
    /// By the processor.
    InSoftware {
        /// Quality setting, lower meaning better.
        quality: u8,
        /// Speed preset, trading time against size.
        preset: String,
    },
    /// On a card that was proved to accept this work.
    ///
    /// A card is driven by a rate rather than by a quality on purpose: every
    /// codec counts quality on its own scale there, and the same number means
    /// three different things across the three codecs a card produces. A rate
    /// means the same thing to all of them.
    OnACard {
        card: Card,
        /// Whether the card reads the film for itself as well.
        ///
        /// Only ever true of a codec it was proved to read. It changes the
        /// whole shape of the invocation rather than adding an option: the
        /// picture then arrives already on the card, and everything that would
        /// have handed it up is work that must not be asked for.
        reads_the_film: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoEncode {
    /// Encoder name, as the tool knows it.
    pub encoder: String,
    /// Who does the work.
    pub how: Rebuilding,
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
    /// Paint this subtitle onto every frame, by its place in the file.
    ///
    /// Only ever a subtitle made of pictures. There is no text in one to hand
    /// a browser, so painting it on is the only way to show it, and that is
    /// what makes such a subtitle cost a full rebuild.
    ///
    /// A subtitle made of words is never painted on here. The tool paints
    /// pictures onto pictures, and drawing words would mean naming the film
    /// inside a filter expression, where a colon or an apostrophe in a title
    /// changes what the expression means. It stays alongside the picture,
    /// which is also the only form a viewer can switch off.
    pub burn_in_subtitle: Option<i32>,
}

/// How often the tool is asked to say where it has got to, in seconds.
///
/// Ten times a second. A segment is handed over once the tool has said it
/// passed the end of it, so this is the delay between a segment being finished
/// and anybody knowing. The default of half a second or more was measured as
/// most of that wait on a real film.
const HOW_OFTEN_IT_REPORTS: &str = "0.1";

/// Quality the software encoder aims at, on its own scale.
///
/// The usual middle of the road: visually indistinguishable from the source on
/// film material, and small enough that a local network never notices.
const SOFTWARE_QUALITY: u8 = 23;

/// Speed the software encoder works at.
///
/// Fast enough to stay ahead of a viewer on a processor with other things to
/// do, which is the only thing that matters while somebody is watching.
const SOFTWARE_PRESET: &str = "veryfast";

impl VideoEncode {
    /// Software encoding to the codec every browser reads.
    pub fn software_h264() -> Self {
        Self {
            encoder: "libx264".to_string(),
            how: Rebuilding::InSoftware {
                quality: SOFTWARE_QUALITY,
                preset: SOFTWARE_PRESET.to_string(),
            },
            max_bitrate: None,
            scale_to_height: None,
            tone_map: false,
            keyframe_interval: None,
            burn_in_subtitle: None,
        }
    }

    /// Encoding one codec on a card.
    ///
    /// Absent when this card was not proved to produce that codec: a card that
    /// refused a codec at start-up is a card that will refuse it in the middle
    /// of a film.
    pub fn on_a_card(card: &Card, codec: &str, reads_the_film: bool) -> Option<Self> {
        let encoder = card.encoder_for(codec)?.to_string();
        Some(Self {
            encoder,
            how: Rebuilding::OnACard {
                card: card.clone(),
                reads_the_film,
            },
            max_bitrate: None,
            scale_to_height: None,
            tone_map: false,
            keyframe_interval: None,
            burn_in_subtitle: None,
        })
    }

    /// The card doing the work, and whether it reads the film as well.
    pub fn card(&self) -> Option<(&Card, bool)> {
        match &self.how {
            Rebuilding::OnACard {
                card,
                reads_the_film,
            } => Some((card, *reads_the_film)),
            Rebuilding::InSoftware { .. } => None,
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
        /// Where the tool is to cut.
        cut: WhereToCut,
        /// Number the first produced segment carries, so that a jump starts
        /// the encoder further in rather than from the beginning.
        start_number: u32,
    },
    /// A single image, for a thumbnail.
    StillImage(PathBuf),
    /// Nothing kept, used when only the measurement matters.
    Discard,
}

/// Where the tool is to cut one segment from the next.
///
/// The server writes the playlist before anything is produced, so it has to
/// know where every cut will fall. That only holds if the tool has one way of
/// answering, and the tool has two: it aims for a length, and it can only cut
/// where the picture stands on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhereToCut {
    /// At every place the film can be started, and nowhere else.
    ///
    /// For a picture carried over untouched, where the server chooses none of
    /// the cuts. It is the only rule whose answer does not depend on where the
    /// tool was set going, and a jump sets it going anywhere: the length the
    /// tool aims for advances by a fixed step at every cut rather than being
    /// measured from the cut it just made, so a film with two starting points
    /// close together comes out cut differently depending on where the reading
    /// began. Asked for a length shorter than any gap between two starting
    /// points, the tool has no choice left and cuts at all of them.
    AtEveryKeyFrame,
    /// On a grid of this length.
    ///
    /// For a picture the server rebuilds, which is given a starting point on
    /// every boundary of that same grid: the tool then has a place to cut
    /// exactly where the playlist says, and no reason to cut anywhere else.
    Every(Millis),
}

/// The length handed to the tool for a cut at every starting point.
///
/// A millisecond is under one frame of any film there is, so the length the
/// tool aims for is behind from the first cut onwards and stays behind. Zero
/// is not used: the tool reads it as "unset" and falls back to its own
/// default.
const SHORTER_THAN_ANY_PICTURE: Millis = Millis::new(1);

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

    /// The card this command works on, when it works on one.
    pub fn card(&self) -> Option<(&Card, bool)> {
        match &self.video {
            VideoOutput::Encode(encode) => encode.card(),
            _ => None,
        }
    }

    /// The filter graph that paints a subtitle onto every frame, when one is
    /// being drawn in.
    ///
    /// Everything else the picture goes through happens first, on the picture
    /// alone: painting the words on and then shrinking would shrink the words
    /// with it, which is how subtitles end up unreadable on a small screen.
    fn picture_painted_with_subtitles(&self) -> Option<String> {
        let VideoOutput::Encode(encode) = &self.video else {
            return None;
        };
        let subtitle = encode.burn_in_subtitle?;

        let picture = match self.streams.video_index {
            Some(index) => format!("[0:{index}]"),
            None => "[0:v:0]".to_string(),
        };
        let before = match picture_filter_chain(encode) {
            Some(filters) => format!("{picture}{filters}[picture];[picture]"),
            None => picture,
        };
        Some(format!(
            "{before}[0:{subtitle}]overlay=shortest=0{PAINTED_PICTURE}"
        ))
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

        // The card is opened before anything is read: a device named after the
        // input is a device the filters cannot reach, and the tool says so in
        // a sentence that names neither.
        if let Some((card, reads_the_film)) = self.card() {
            for argument in card.opening_arguments(reads_the_film) {
                push!(&argument);
            }
        }

        if self.report_progress {
            push!("-progress");
            push!("pipe:1");
            push!("-nostats");
            // How often it says where it has got to. The default is the best
            // part of a second, and a segment is only known to be finished
            // once the tool has said so: measured on a real film, that wait
            // was most of the time between a segment being written and being
            // handed over. It costs a line of text now and then.
            push!("-stats_period");
            push!(HOW_OFTEN_IT_REPORTS);
        }

        // Seeking before the input makes the tool jump there; after the input
        // it would decode and throw away everything up to that point.
        if let Some(start) = self.input.start_at {
            // Segments carry their own clock, and a player places each one by
            // it. Without this the tool restarts that clock at zero after a
            // jump, so a segment the playlist says covers the eighth second
            // announces itself as the first: measured, and it puts a player
            // in the wrong place.
            if matches!(self.output, Output::Segments { .. }) {
                push!("-copyts");

                // A copied picture can only begin at a key frame, so the tool
                // rewinds to the one before the position asked for. Left to
                // itself it then trims the sound to that position, because the
                // sound is being decoded and can start anywhere. The segment
                // then carries a picture from one moment and a sound from
                // another: measured at ten seconds apart on a film with key
                // frames ten seconds apart, which is a film with no sound on
                // it as far as anyone watching is concerned.
                //
                // Asking for no trimming keeps them together. Only ever when
                // the picture is copied: a picture being rebuilt starts
                // exactly where it was asked to, and trimming is what makes
                // that true.
                if matches!(self.video, VideoOutput::Copy) {
                    push!("-noaccurate_seek");
                }
            }
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

        // Drawing a subtitle into the picture is a filter with two inputs, and
        // a filter with two inputs cannot be written as a plain video filter.
        // The whole chain moves to a named graph, and the picture is then
        // mapped by the name that graph gives it rather than by its number.
        let painted = self.picture_painted_with_subtitles();
        if let Some(graph) = &painted {
            push!("-filter_complex");
            push!(graph);
            push!("-map");
            push!(PAINTED_PICTURE);
        }

        match (self.streams.video_index, &self.video) {
            _ if painted.is_some() => {}
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

        // A subtitle being drawn into the picture is consumed by the graph:
        // carrying it out as a track of its own as well would show it twice.
        if let (Some(index), None) = (self.streams.subtitle_index, &painted) {
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
                // Already written into the named graph when the picture is
                // being painted with subtitles: saying it twice would apply
                // it twice.
                if painted.is_none() {
                    if let Some(filters) = picture_filter_chain(encode) {
                        push!("-vf");
                        push!(&filters);
                    }
                }

                push!("-c:v");
                push!(&encode.encoder);

                match &encode.how {
                    Rebuilding::InSoftware { quality, preset } => {
                        push!("-preset");
                        push!(preset);
                        push!("-crf");
                        push!(&quality.to_string());
                    }
                    // A rate rather than a quality, because the three codecs a
                    // card produces count quality on three different scales
                    // and a rate means the same thing to all of them.
                    Rebuilding::OnACard { .. } => {
                        if let Some(bitrate) = encode.max_bitrate {
                            push!("-b:v");
                            push!(&bitrate.to_string());
                        }
                    }
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
                // Only ever in software: a picture sitting on a card is not
                // held in a layout the processor names, and asking for one is
                // how a card refuses a film it was perfectly able to rebuild.
                if encode.card().is_none() {
                    push!("-pix_fmt");
                    push!("yuv420p");
                }
            }
        }

        match &self.audio {
            AudioOutput::None => {}
            AudioOutput::Copy => {
                push!("-c:a");
                push!("copy");
            }
            AudioOutput::Encode(encode) => {
                if let Some(filter) = self.sound_filters(encode) {
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
                cut,
                start_number,
            } => {
                push!("-f");
                push!("hls");
                push!("-hls_time");
                push!(&format_seconds(match cut {
                    WhereToCut::AtEveryKeyFrame => SHORTER_THAN_ANY_PICTURE,
                    WhereToCut::Every(length) => *length,
                }));
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

    /// Everything applied to the sound, the silence filling a hole at the
    /// front included.
    fn sound_filters(&self, encode: &AudioEncode) -> Option<String> {
        let mut stages: Vec<String> = Vec::new();
        if self.fills_the_hole_before_the_sound() {
            stages.push(SILENCE_UP_TO_THE_PICTURE.to_string());
        }
        stages.extend(audio_filter_chain(encode));
        (!stages.is_empty()).then(|| stages.join(","))
    }

    /// Whether the sound is to be padded up to where the picture begins.
    ///
    /// A film whose sound begins seconds after its picture has a hole at the
    /// front, and a browser handed a stream with a hole in it does not wait
    /// through it: it plays the sound it was given as soon as it has it, and
    /// the film then runs with the sound ahead of the picture from end to end.
    /// Measured on the maintainer's collection: a film whose French track
    /// begins three seconds in played three seconds ahead of itself, while a
    /// desktop player reading the same file over the network was perfectly in
    /// step. The tool was right all along; what it produced was a stream no
    /// browser could place.
    ///
    /// Only a run that begins at the beginning of the film, because that is
    /// the only place such a hole exists: a run set going part way through
    /// lands in the middle of a sound that has been playing for an hour, and
    /// asking for the front to be filled there would ask for that hour to be
    /// filled with silence.
    ///
    /// Only a sound being rebuilt, too. A sound carried over untouched cannot
    /// be filtered at all, and a film served that way keeps its hole.
    fn fills_the_hole_before_the_sound(&self) -> bool {
        matches!(self.output, Output::Segments { .. }) && self.input.start_at.is_none()
    }
}

/// Fills the front of the sound with silence so that it begins with the
/// picture.
///
/// The sound keeps its own moment: what was at three seconds is still at
/// three seconds, and the three seconds before it are silence rather than
/// nothing at all.
const SILENCE_UP_TO_THE_PICTURE: &str = "aresample=first_pts=0";

/// Filter that maps a wide gamut picture into standard range.
///
/// Goes through a linear light stage rather than converting directly, which is
/// what keeps skin tones from turning grey. Used for streams and for every
/// still image pulled out of a file, because an unconverted frame is exactly
/// the washed out thumbnail seen on other servers.
pub(crate) const TONE_MAP_FILTER: &str = concat!(
    "zscale=transfer=linear:npl=100,",
    "format=gbrpf32le,",
    "zscale=primaries=bt709,",
    "tonemap=tonemap=hable:desat=0,",
    "zscale=transfer=bt709:matrix=bt709:range=limited,",
    "format=yuv420p"
);

/// What the painted picture is called inside the filter graph.
const PAINTED_PICTURE: &str = "[painted]";

/// Builds what happens to the picture before it is encoded.
///
/// Two shapes, because the two paths have nothing in common beyond the order:
/// on a card the picture is handed up and worked on there, in software it is
/// worked on where it already is.
fn picture_filter_chain(encode: &VideoEncode) -> Option<String> {
    if let Some((card, reads_the_film)) = encode.card() {
        let filters = card.filters_for(encode.scale_to_height, encode.tone_map, reads_the_film);
        return (!filters.is_empty()).then(|| filters.join(","));
    }

    let mut filters: Vec<String> = Vec::new();

    // Made smaller first, converted afterwards. Converting colours is the most
    // expensive thing done to a picture, since every pixel passes through a
    // stage in floating point, so doing it on a quarter of the pixels costs a
    // quarter of the work. Measured on a four thread machine with a real wide
    // gamut film: converting at full size ran at a third of real time, which
    // is a slideshow, and shrinking first ran faster than real time.
    //
    // The other order is marginally more correct on paper, the picture being
    // resized while still in its wide range. Nobody can see the difference,
    // and everybody can see a film that stutters.
    if let Some(height) = encode.scale_to_height {
        filters.push(format!("scale=-2:{height}"));
    }
    if encode.tone_map {
        filters.push(TONE_MAP_FILTER.to_string());
    }
    (!filters.is_empty()).then(|| filters.join(","))
}

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

    /// A command that draws the subtitle at `subtitle` into the picture.
    fn painting(subtitle: i32) -> Command {
        let mut encode = VideoEncode::software_h264();
        encode.burn_in_subtitle = Some(subtitle);
        Command::new(
            Input::new("/media/film.mkv"),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        )
        .with_video(VideoOutput::Encode(encode))
    }

    #[test]
    fn a_subtitle_drawn_into_the_picture_is_a_graph_and_not_a_plain_filter() {
        // A filter with two inputs cannot be written as a plain video filter,
        // and a picture coming out of a named graph is mapped by that name.
        let args = arguments(&painting(3));
        let graph = position(&args, "-filter_complex").expect("a graph is built");
        assert_eq!(args[graph + 1], "[0:v:0][0:3]overlay=shortest=0[painted]");

        let mapped = position(&args, "-map").expect("the picture is mapped");
        assert_eq!(args[mapped + 1], "[painted]");
        assert!(
            !args.iter().any(|value| value == "-vf"),
            "saying it twice would apply it twice: {args:?}"
        );
    }

    #[test]
    fn what_the_picture_goes_through_happens_before_the_words_are_painted_on() {
        // Painting the words on and then shrinking would shrink the words with
        // it, which is how subtitles end up unreadable on a small screen.
        let mut encode = VideoEncode::software_h264();
        encode.burn_in_subtitle = Some(3);
        encode.scale_to_height = Some(720);
        let command = Command::new(
            Input::new("/media/film.mkv"),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        )
        .with_video(VideoOutput::Encode(encode));

        let args = arguments(&command);
        let graph = position(&args, "-filter_complex").expect("a graph is built");
        assert_eq!(
            args[graph + 1],
            "[0:v:0]scale=-2:720[picture];[picture][0:3]overlay=shortest=0[painted]"
        );
        assert!(
            !args.iter().any(|value| value == "-vf"),
            "the scaling is already in the graph: saying it again would shrink \
             the picture twice: {args:?}"
        );
    }

    #[test]
    fn a_subtitle_being_painted_on_is_not_also_carried_out_as_a_track() {
        // It is consumed by the graph; carrying it as well would show it twice.
        let command = painting(3).with_streams(StreamSelection {
            video_index: Some(0),
            audio_index: Some(1),
            subtitle_index: Some(3),
        });
        let args = arguments(&command);
        assert!(
            !args.iter().any(|value| value == "0:3"),
            "the subtitle is in the picture, not beside it: {args:?}"
        );
        let graph = position(&args, "-filter_complex").expect("a graph is built");
        assert_eq!(args[graph + 1], "[0:0][0:3]overlay=shortest=0[painted]");
    }

    #[test]
    fn nothing_changes_for_a_film_with_no_subtitle_to_paint_on() {
        let mut encode = VideoEncode::software_h264();
        encode.scale_to_height = Some(720);
        let command = Command::new(
            Input::new("/media/film.mkv"),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        )
        .with_video(VideoOutput::Encode(encode));

        let args = arguments(&command);
        assert!(!args.iter().any(|value| value == "-filter_complex"));
        let filters = position(&args, "-vf").expect("the picture is still scaled");
        assert_eq!(args[filters + 1], "scale=-2:720");
    }

    #[test]
    fn a_picture_is_made_smaller_before_its_colours_are_converted() {
        // The other way round was the rule here, on the grounds that resizing
        // a picture while it is still in its wide range keeps it cleaner. It
        // does, on paper. Measured on a four thread machine with a real wide
        // gamut film, it also ran at a third of real time, which is a
        // slideshow with sound, against faster than real time this way.
        // Nobody can see the difference; everybody can see a film stutter.
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
            scale < tone_map,
            "converting every pixel of a picture about to be thrown away is \
             three quarters of the work for nothing: {filters}"
        );
    }

    /// A card that produces all three codecs and can do everything asked of
    /// it, which is what the maintainer's card is.
    fn a_card() -> Card {
        Card {
            way: crate::capabilities::HardwareAcceleration::Vaapi,
            device: PathBuf::from("/dev/dri/renderD128"),
            encoders: [
                ("h264", "h264_vaapi"),
                ("hevc", "hevc_vaapi"),
                ("av1", "av1_vaapi"),
            ]
            .into_iter()
            .map(|(codec, encoder)| (codec.to_string(), encoder.to_string()))
            .collect(),
            decoders: ["h264", "hevc", "av1"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            can_scale: true,
            can_tone_map: true,
        }
    }

    #[test]
    fn a_card_is_opened_before_the_film_is_read() {
        // A device named after the input is a device the filters cannot reach,
        // and the tool then refuses in a sentence naming neither.
        let mut encode =
            VideoEncode::on_a_card(&a_card(), "av1", false).expect("this card produces it");
        encode.max_bitrate = Some(8_000_000);
        let command = Command::new(
            Input::new("/media/film.mkv").starting_at(Millis::new(20_000)),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        )
        .with_video(VideoOutput::Encode(encode));

        let args = arguments(&command);
        let opened = position(&args, "-init_hw_device").expect("the card is opened");
        assert!(opened < position(&args, "-i").expect("an input is present"));
        assert_eq!(args[opened + 1], "vaapi=card:/dev/dri/renderD128");
        assert!(args.contains(&"av1_vaapi".to_string()));
    }

    #[test]
    fn a_card_is_never_given_the_settings_of_a_software_encoder() {
        // Neither accepts the other's, and a card handed a speed preset or a
        // picture layout it does not hold refuses a film it was perfectly able
        // to rebuild.
        let mut encode =
            VideoEncode::on_a_card(&a_card(), "h264", false).expect("this card produces it");
        encode.max_bitrate = Some(6_000_000);
        let args = arguments(
            &Command::new(
                Input::new("/media/film.mkv"),
                Output::File(PathBuf::from("/tmp/out.mp4")),
            )
            .with_video(VideoOutput::Encode(encode)),
        );

        assert!(!args.iter().any(|value| value == "-preset"), "{args:?}");
        assert!(!args.iter().any(|value| value == "-crf"), "{args:?}");
        assert!(!args.iter().any(|value| value == "-pix_fmt"), "{args:?}");

        let rate = position(&args, "-b:v").expect("a card is driven by a rate");
        assert_eq!(args[rate + 1], "6000000");
        assert_eq!(
            args[position(&args, "-bufsize").expect("a buffer goes with it") + 1],
            "12000000"
        );
    }

    #[test]
    fn a_picture_rebuilt_on_a_card_is_handed_up_to_it_first() {
        let mut encode =
            VideoEncode::on_a_card(&a_card(), "hevc", false).expect("this card produces it");
        encode.scale_to_height = Some(1080);
        encode.tone_map = true;
        let args = arguments(
            &Command::new(
                Input::new("/media/film.mkv"),
                Output::File(PathBuf::from("/tmp/out.mp4")),
            )
            .with_video(VideoOutput::Encode(encode)),
        );

        let filters = &args[position(&args, "-vf").expect("a filter chain is present") + 1];
        assert_eq!(
            filters,
            "format=p010,hwupload,scale_vaapi=w=-2:h=1080,tonemap_vaapi=format=nv12"
        );
        assert!(
            !filters.contains("zscale"),
            "converting colours on the processor is exactly the work the card exists to take: {filters}"
        );
    }

    #[test]
    fn a_card_that_reads_the_film_is_never_asked_to_be_handed_it_as_well() {
        // The picture then arrives already up there. Asking to hand it up
        // again is how a card refuses a film it was reading perfectly well.
        let mut encode = VideoEncode::on_a_card(&a_card(), "av1", true).expect("proved");
        encode.scale_to_height = Some(1080);
        encode.tone_map = true;
        let args = arguments(
            &Command::new(
                Input::new("/media/film.mkv"),
                Output::File(PathBuf::from("/tmp/out.mp4")),
            )
            .with_video(VideoOutput::Encode(encode)),
        );

        let reading = position(&args, "-hwaccel").expect("the card reads the film");
        assert!(
            reading < position(&args, "-i").expect("an input is present"),
            "reading is an option of the input, and one placed after it applies to nothing"
        );
        assert_eq!(
            args[position(&args, "-hwaccel_output_format").expect("frames stay on the card") + 1],
            "vaapi",
            "without this the card hands every frame back down, which is the whole cost"
        );

        let filters = &args[position(&args, "-vf").expect("a filter chain is present") + 1];
        assert_eq!(filters, "scale_vaapi=w=-2:h=1080,tonemap_vaapi=format=nv12");
        assert!(!filters.contains("hwupload"), "{filters}");
    }

    #[test]
    fn a_film_the_card_cannot_read_is_still_rebuilt_on_it() {
        // Decoded by the processor and handed up. That still moves the
        // expensive half of the work, and it is what works whatever the film
        // holds.
        let encode = VideoEncode::on_a_card(&a_card(), "h264", false).expect("proved");
        let args = arguments(
            &Command::new(
                Input::new("/media/film.mkv"),
                Output::File(PathBuf::from("/tmp/out.mp4")),
            )
            .with_video(VideoOutput::Encode(encode)),
        );

        assert!(!args.iter().any(|value| value == "-hwaccel"), "{args:?}");
        assert!(args.contains(&"-init_hw_device".to_string()));
        assert_eq!(
            args[position(&args, "-vf").expect("a filter chain is present") + 1],
            "format=nv12,hwupload",
            "handed up in the layout the encoder takes, and nothing more asked of the card"
        );
    }

    #[test]
    fn a_film_the_card_read_itself_is_put_in_a_layout_the_encoder_takes() {
        // Measured on a real card: it read the film perfectly well and then
        // offered the frames with ten bits to a channel, which the encoder
        // refuses. As far as a viewer is concerned the card had not read it.
        let encode = VideoEncode::on_a_card(&a_card(), "av1", true).expect("proved");
        let args = arguments(
            &Command::new(
                Input::new("/media/film.mkv"),
                Output::File(PathBuf::from("/tmp/out.mp4")),
            )
            .with_video(VideoOutput::Encode(encode)),
        );
        assert_eq!(
            args[position(&args, "-vf").expect("a filter chain is present") + 1],
            "scale_vaapi=format=nv12"
        );
    }

    #[test]
    fn a_card_that_was_never_proved_to_produce_a_codec_is_not_asked_for_it() {
        // A card that refused a codec at start-up is a card that refuses it in
        // the middle of a film, and by then somebody is watching.
        let mut card = a_card();
        card.encoders.remove("av1");
        assert!(VideoEncode::on_a_card(&card, "av1", false).is_none());
        assert!(VideoEncode::on_a_card(&card, "h264", false).is_some());
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
                cut: WhereToCut::Every(Millis::new(4000)),
                start_number: 312,
            },
        );
        let args = arguments(&command);
        let index = position(&args, "-start_number").expect("a start number is present");
        assert_eq!(args[index + 1], "312");
    }

    /// A stream to a browser, the shape every test below is about.
    fn segments_of(input: Input) -> Command {
        Command::new(
            input,
            Output::Segments {
                pattern: PathBuf::from("/tmp/session/segment-%d.m4s"),
                initialisation: PathBuf::from("/tmp/session/init.mp4"),
                tool_playlist: PathBuf::from("/tmp/session/tool.m3u8"),
                cut: WhereToCut::Every(Millis::new(4000)),
                start_number: 0,
            },
        )
        .with_audio(AudioOutput::Encode(AudioEncode::browser_stereo("aac")))
    }

    /// What was asked of the sound, as one string.
    fn sound_filters(command: &Command) -> Option<String> {
        let args = arguments(command);
        let at = args.iter().position(|value| value == "-af")?;
        args.get(at + 1).cloned()
    }

    #[test]
    fn a_film_served_from_its_beginning_has_its_sound_filled_up_to_its_picture() {
        // A film whose sound begins after its picture leaves a hole at the
        // front, and a browser plays across it rather than waiting through it.
        let filters = sound_filters(&segments_of(Input::new("/media/film.mkv")))
            .expect("the sound is filtered");
        assert!(
            filters.starts_with("aresample=first_pts=0"),
            "the filling comes first, so everything after it works on a sound \
             that already begins with the picture: {filters}"
        );
    }

    #[test]
    fn a_run_set_going_part_way_through_never_fills_anything() {
        // There is no hole in the middle of a film, and asking for one to be
        // filled there asks for everything up to that point to be silence.
        let filters = sound_filters(&segments_of(
            Input::new("/media/film.mkv").starting_at(Millis::new(1_340_000)),
        ));
        assert!(
            !filters.unwrap_or_default().contains("first_pts"),
            "nothing was skipped over at the front, so nothing is missing"
        );
    }

    #[test]
    fn a_file_written_out_is_never_filled_either() {
        // A file is opened by whatever opens it, and every such player places
        // a sound by what the file says rather than by what it was handed.
        let command = Command::new(
            Input::new("/media/film.mkv"),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        )
        .with_audio(AudioOutput::Encode(AudioEncode::browser_stereo("aac")));
        assert!(!sound_filters(&command)
            .unwrap_or_default()
            .contains("first_pts"));
    }

    #[test]
    fn a_sound_carried_over_untouched_is_never_filtered() {
        let command = segments_of(Input::new("/media/film.mkv")).with_audio(AudioOutput::Copy);
        assert!(
            sound_filters(&command).is_none(),
            "a sound nobody rebuilds cannot be filtered at all"
        );
    }

    #[test]
    fn a_jump_keeps_the_clock_of_the_film_so_a_player_places_the_segment_right() {
        let jumped = Command::new(
            Input::new("/media/film.mkv").starting_at(Millis::new(8_000)),
            Output::Segments {
                pattern: PathBuf::from("/tmp/session/segment-%d.m4s"),
                initialisation: PathBuf::from("/tmp/session/init.mp4"),
                tool_playlist: PathBuf::from("/tmp/session/tool.m3u8"),
                cut: WhereToCut::Every(Millis::new(4000)),
                start_number: 2,
            },
        );
        let args = arguments(&jumped);
        assert!(args.contains(&"-copyts".to_string()));
        assert!(
            position(&args, "-copyts") < position(&args, "-i"),
            "it has to reach the reading of the file, not the writing"
        );

        let from_the_start = Command::new(
            Input::new("/media/film.mkv"),
            Output::Segments {
                pattern: PathBuf::from("/tmp/session/segment-%d.m4s"),
                initialisation: PathBuf::from("/tmp/session/init.mp4"),
                tool_playlist: PathBuf::from("/tmp/session/tool.m3u8"),
                cut: WhereToCut::Every(Millis::new(4000)),
                start_number: 0,
            },
        );
        assert!(
            !arguments(&from_the_start).contains(&"-copyts".to_string()),
            "nothing was skipped, so there is no clock to preserve"
        );

        let plain_file = Command::new(
            Input::new("/media/film.mkv").starting_at(Millis::new(8_000)),
            Output::File(PathBuf::from("/tmp/out.mp4")),
        );
        assert!(
            !arguments(&plain_file).contains(&"-copyts".to_string()),
            "a file that starts part way through starts at nothing, as a file does"
        );
    }

    #[test]
    fn after_a_jump_a_copied_picture_keeps_its_sound_with_it() {
        // Measured, on a film with key frames ten seconds apart: the picture
        // began at the key frame before the jump and the sound ten seconds
        // later, in the same segment. It only happens when the picture is
        // copied and the sound is rebuilt, which is the commonest film of all:
        // a picture any browser reads and a soundtrack none of them do.
        let segments = || Output::Segments {
            pattern: PathBuf::from("/tmp/session/segment-%d.m4s"),
            initialisation: PathBuf::from("/tmp/session/init.mp4"),
            tool_playlist: PathBuf::from("/tmp/session/tool.m3u8"),
            cut: WhereToCut::Every(Millis::new(4000)),
            start_number: 5,
        };

        let copied = Command::new(
            Input::new("/media/film.mkv").starting_at(Millis::new(20_000)),
            segments(),
        )
        .with_video(VideoOutput::Copy)
        .with_audio(AudioOutput::Encode(AudioEncode::browser_stereo("aac")));
        let args = arguments(&copied);
        assert!(args.contains(&"-noaccurate_seek".to_string()));
        assert!(
            position(&args, "-noaccurate_seek") < position(&args, "-i"),
            "it has to reach the reading of the file, not the writing"
        );

        let rebuilt = Command::new(
            Input::new("/media/film.mkv").starting_at(Millis::new(20_000)),
            segments(),
        )
        .with_video(VideoOutput::Encode(VideoEncode {
            keyframe_interval: Some(Millis::new(4000)),
            ..VideoEncode::software_h264()
        }));
        assert!(
            !arguments(&rebuilt).contains(&"-noaccurate_seek".to_string()),
            "a picture being rebuilt starts exactly where it was asked to, and trimming is what makes that true"
        );

        let from_the_start =
            Command::new(Input::new("/media/film.mkv"), segments()).with_video(VideoOutput::Copy);
        assert!(
            !arguments(&from_the_start).contains(&"-noaccurate_seek".to_string()),
            "nothing was skipped, so there is nothing to trim"
        );
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
                cut: WhereToCut::Every(Millis::new(4000)),
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

        // Often enough that a finished segment is known to be finished. What
        // the tool does by default was measured as most of the wait between a
        // segment being written and being handed over.
        let period = position(&args, "-stats_period").expect("a period is asked for");
        assert_eq!(args[period + 1], "0.1");
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
