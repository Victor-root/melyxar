//! What the application's own player of music heard of its sound, under the
//! `music-app` tag: the phone's, the car's through it, the television's.
//!
//! Kept apart from the page's tags on purpose. The application plays the
//! music itself, away from any page, and a sound that breaks up in a car is
//! chased through what only the device knows: the sound card running dry,
//! the network making it wait, how fast each song came in, what took the
//! sound away for a moment, where the sound goes out, and every ten seconds
//! how it is holding up. Each a fact this module names, as everywhere else in
//! the journal; every word from outside is read against a list or cut short.

use serde::Deserialize;

use super::Said;
use crate::page::cut_to;

/// How much of a failure's words is written down.
const ENOUGH_OF_AN_ERROR: usize = 200;

/// How much of the name of a form of sound is written down.
const ENOUGH_OF_A_FORM: usize = 40;

/// The most ways out of the sound one line names.
const MOST_OUTPUTS: usize = 12;

/// What the application saw, each with what it says.
#[derive(Debug, Deserialize)]
#[serde(tag = "what", rename_all = "snake_case")]
pub(super) enum Fact {
    RanDry(RanDry),
    Waited(Waited),
    Fetched(Fetched),
    FetchFailed(FetchFailed),
    SoundRead(SoundRead),
    SoundOut(SoundOut),
    SoundFailed(SoundFailed),
    Focus(Focus),
    Outputs(Outputs),
    Health(Health),
    Jumped(Jumped),
    HandedOver(HandedOver),
}

/// The sound card was given nothing in time and played a silence.
#[derive(Debug, Deserialize)]
pub(super) struct RanDry {
    deck: u8,
    live: bool,
    second: f64,
    /// How much sound the card holds, in milliseconds.
    holds_ms: i64,
    /// How long since it was last given any.
    since_fed_ms: i64,
    /// How much of the song was in hand past where it plays, in seconds.
    ahead_seconds: f64,
    /// How many more were not told, the device saying a few at most.
    #[serde(default)]
    untold: u32,
}

/// The song stopped to wait for its sound, and went on.
#[derive(Debug, Deserialize)]
pub(super) struct Waited {
    deck: u8,
    live: bool,
    second: f64,
    waited_ms: i64,
}

/// A piece of a song that came in, and how fast.
#[derive(Debug, Deserialize)]
pub(super) struct Fetched {
    deck: u8,
    kilobytes: i64,
    ms: i64,
}

/// A piece of a song that did not come in.
#[derive(Debug, Deserialize)]
pub(super) struct FetchFailed {
    deck: u8,
    error: String,
    /// Given up by the player rather than lost.
    cancelled: bool,
}

/// The form of sound a deck reads.
#[derive(Debug, Deserialize)]
pub(super) struct SoundRead {
    deck: u8,
    form: String,
    #[serde(default)]
    sample_rate: Option<i32>,
    #[serde(default)]
    channels: Option<i32>,
    #[serde(default)]
    bitrate: Option<i32>,
}

/// What a deck hands to the sound card.
#[derive(Debug, Deserialize)]
pub(super) struct SoundOut {
    deck: u8,
    encoding: i32,
    sample_rate: i32,
    channels: i32,
    /// How much sound the card is given at a time, in bytes.
    buffer_bytes: i32,
    /// Handed to a part of the device that plays it alone.
    offload: bool,
}

/// Where the sound broke.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Broke {
    Decoding,
    Output,
    Player,
}

impl Broke {
    fn as_word(self) -> &'static str {
        match self {
            Self::Decoding => "reading its sound",
            Self::Output => "handing it to the sound card",
            Self::Player => "playing it",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct SoundFailed {
    deck: u8,
    during: Broke,
    error: String,
}

/// What somebody else did to the sound.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Change {
    /// Given back.
    Gain,
    /// Taken for good, by another player.
    Loss,
    /// Taken for a moment: a call, a guide speaking.
    LossTransient,
    /// Lowered for a moment under somebody's few words.
    Duck,
}

impl Change {
    fn as_word(self) -> &'static str {
        match self {
            Self::Gain => "given back",
            Self::Loss => "taken for good",
            Self::LossTransient => "taken for a moment",
            Self::Duck => "lowered for a moment",
        }
    }
}

/// What the player did about it.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Answer {
    Paused,
    Lowered,
    Raised,
    PlayedAgain,
    Nothing,
}

impl Answer {
    fn as_word(self) -> &'static str {
        match self {
            Self::Paused => "paused",
            Self::Lowered => "lowered",
            Self::Raised => "raised back",
            Self::PlayedAgain => "played again",
            Self::Nothing => "left as it was",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct Focus {
    change: Change,
    did: Answer,
}

/// A way the sound can go out of the device.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Output {
    Speaker,
    Earpiece,
    Wired,
    BluetoothA2dp,
    BluetoothSco,
    BluetoothLe,
    Usb,
    Hdmi,
    /// A car's own sound system, as cars running Android name it.
    Bus,
    RemoteSubmix,
    Other,
}

impl Output {
    fn as_word(self) -> &'static str {
        match self {
            Self::Speaker => "speaker",
            Self::Earpiece => "earpiece",
            Self::Wired => "wired",
            Self::BluetoothA2dp => "bluetooth",
            Self::BluetoothSco => "bluetooth call",
            Self::BluetoothLe => "bluetooth le",
            Self::Usb => "usb",
            Self::Hdmi => "hdmi",
            Self::Bus => "car",
            Self::RemoteSubmix => "relayed",
            Self::Other => "other",
        }
    }
}

fn outputs_written(outputs: &[Output]) -> String {
    outputs
        .iter()
        .take(MOST_OUTPUTS)
        .map(|output| output.as_word())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The ways out of the sound, said when they change.
#[derive(Debug, Deserialize)]
pub(super) struct Outputs {
    outputs: Vec<Output>,
    in_car: bool,
}

/// How hot the device says it is running, as it names it.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Heat {
    None,
    Light,
    Moderate,
    Severe,
    Critical,
    Emergency,
    Shutdown,
    Unknown,
}

impl Heat {
    fn as_word(self) -> &'static str {
        match self {
            Self::None => "cool",
            Self::Light => "warm",
            Self::Moderate => "hot",
            Self::Severe => "too hot",
            Self::Critical => "critical",
            Self::Emergency => "emergency",
            Self::Shutdown => "shutting down",
            Self::Unknown => "not said",
        }
    }
}

/// How the sound held up over the last few seconds.
#[derive(Debug, Deserialize)]
pub(super) struct Health {
    over_ms: i64,
    playing: bool,
    second: f64,
    ahead_seconds: f64,
    ran_dry: u32,
    /// The longest any piece of sound took to be brought to its level, and
    /// all of them together, in milliseconds.
    level_longest_ms: f64,
    level_busy_ms: f64,
    heat: Heat,
    battery_saver: bool,
    in_car: bool,
    outputs: Vec<Output>,
}

/// Why a deck moved.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Moved {
    Seek,
    SeekAdjusted,
    Skipped,
    Removed,
    Other,
}

impl Moved {
    fn as_word(self) -> &'static str {
        match self {
            Self::Seek => "asked",
            Self::SeekAdjusted => "set right by the player",
            Self::Skipped => "skipped by the player",
            Self::Removed => "what it played taken away",
            Self::Other => "for another reason",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct Jumped {
    deck: u8,
    from_second: f64,
    to_second: f64,
    why: Moved,
}

/// The next song taking over from the one before.
#[derive(Debug, Deserialize)]
pub(super) struct HandedOver {
    from: u8,
    to: u8,
    /// How late it took over past its moment, in milliseconds.
    late_ms: i64,
    fade_seconds: f64,
}

pub(super) fn write(by: &Said, fact: &Fact) {
    let account = by.account.as_str();
    let song = by.song.as_ref().map(tracing::field::display);
    match fact {
        Fact::RanDry(seen) => tracing::info!(
            account,
            song,
            deck = seen.deck,
            live = seen.live,
            second = seen.second,
            holds_ms = seen.holds_ms,
            since_fed_ms = seen.since_fed_ms,
            ahead_seconds = seen.ahead_seconds,
            untold = seen.untold,
            "the sound card ran dry and played a silence"
        ),
        Fact::Waited(seen) => tracing::info!(
            account,
            song,
            deck = seen.deck,
            live = seen.live,
            second = seen.second,
            waited_ms = seen.waited_ms,
            "the song stopped to wait for its sound, and went on"
        ),
        Fact::Fetched(seen) => tracing::debug!(
            account,
            song,
            deck = seen.deck,
            kilobytes = seen.kilobytes,
            ms = seen.ms,
            kilobits_a_second = (seen.kilobytes * 8 * 1000).checked_div(seen.ms),
            "a piece of a song came in"
        ),
        Fact::FetchFailed(seen) => tracing::info!(
            account,
            song,
            deck = seen.deck,
            cancelled = seen.cancelled,
            error = cut_to(&seen.error, ENOUGH_OF_AN_ERROR),
            "a piece of a song did not come in"
        ),
        Fact::SoundRead(seen) => tracing::debug!(
            account,
            song,
            deck = seen.deck,
            form = cut_to(&seen.form, ENOUGH_OF_A_FORM),
            sample_rate = seen.sample_rate,
            channels = seen.channels,
            bitrate = seen.bitrate,
            "a deck reads this form of sound"
        ),
        Fact::SoundOut(seen) => tracing::debug!(
            account,
            song,
            deck = seen.deck,
            encoding = seen.encoding,
            sample_rate = seen.sample_rate,
            channels = seen.channels,
            buffer_bytes = seen.buffer_bytes,
            offload = seen.offload,
            "a deck hands its sound to the sound card"
        ),
        Fact::SoundFailed(seen) => tracing::warn!(
            account,
            song,
            deck = seen.deck,
            error = cut_to(&seen.error, ENOUGH_OF_AN_ERROR),
            "the sound broke while {}",
            seen.during.as_word()
        ),
        Fact::Focus(seen) => tracing::info!(
            account,
            song,
            did = seen.did.as_word(),
            "the sound was {} by somebody else",
            seen.change.as_word()
        ),
        Fact::Outputs(seen) => tracing::info!(
            account,
            in_car = seen.in_car,
            outputs = outputs_written(&seen.outputs),
            "the ways out of the sound changed"
        ),
        Fact::Health(seen) => tracing::debug!(
            account,
            song,
            over_ms = seen.over_ms,
            playing = seen.playing,
            second = seen.second,
            ahead_seconds = seen.ahead_seconds,
            ran_dry = seen.ran_dry,
            level_longest_ms = seen.level_longest_ms,
            level_busy_ms = seen.level_busy_ms,
            heat = seen.heat.as_word(),
            battery_saver = seen.battery_saver,
            in_car = seen.in_car,
            outputs = outputs_written(&seen.outputs),
            "how the sound held up"
        ),
        Fact::Jumped(seen) => tracing::debug!(
            account,
            song,
            deck = seen.deck,
            from_second = seen.from_second,
            to_second = seen.to_second,
            "a deck moved, {}",
            seen.why.as_word()
        ),
        Fact::HandedOver(seen) => tracing::debug!(
            account,
            song,
            from = seen.from,
            to = seen.to,
            late_ms = seen.late_ms,
            fade_seconds = seen.fade_seconds,
            "the next song took over"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(json: &str) -> std::result::Result<Fact, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn each_fact_is_read_by_its_name() {
        let facts = [
            r#"{"what":"ran_dry","deck":0,"live":true,"second":12.5,"holds_ms":40,"since_fed_ms":96,"ahead_seconds":31.2}"#,
            r#"{"what":"waited","deck":0,"live":true,"second":12.5,"waited_ms":800}"#,
            r#"{"what":"fetched","deck":1,"kilobytes":8200,"ms":1900}"#,
            r#"{"what":"fetch_failed","deck":1,"error":"timeout","cancelled":false}"#,
            r#"{"what":"sound_read","deck":0,"form":"audio/flac","sample_rate":44100,"channels":2}"#,
            r#"{"what":"sound_out","deck":0,"encoding":2,"sample_rate":44100,"channels":2,"buffer_bytes":35280,"offload":false}"#,
            r#"{"what":"sound_failed","deck":0,"during":"output","error":"AudioTrack write failed"}"#,
            r#"{"what":"focus","change":"loss_transient","did":"paused"}"#,
            r#"{"what":"outputs","outputs":["speaker","bluetooth_a2dp","usb"],"in_car":true}"#,
            r#"{"what":"health","over_ms":10000,"playing":true,"second":40.1,"ahead_seconds":28.0,"ran_dry":3,
                "level_longest_ms":2.4,"level_busy_ms":61.0,"heat":"light","battery_saver":false,"in_car":true,
                "outputs":["bus"]}"#,
            r#"{"what":"jumped","deck":0,"from_second":10.0,"to_second":50.0,"why":"seek"}"#,
            r#"{"what":"handed_over","from":0,"to":1,"late_ms":12,"fade_seconds":0.0}"#,
        ];
        for json in facts {
            read(json).unwrap_or_else(|error| panic!("{error}: {json}"));
        }
    }

    #[test]
    fn a_word_outside_the_lists_is_refused() {
        for refused in [
            r#"{"what":"anything_else"}"#,
            r#"{"what":"focus","change":"stolen","did":"paused"}"#,
            r#"{"what":"outputs","outputs":["a toaster"],"in_car":false}"#,
            r#"{"what":"jumped","deck":0,"from_second":1,"to_second":2,"why":"magic"}"#,
        ] {
            assert!(read(refused).is_err(), "{refused}");
        }
    }

    #[test]
    fn the_ways_out_are_named_in_words_and_never_too_many() {
        assert_eq!(
            outputs_written(&[Output::Speaker, Output::Bus, Output::BluetoothA2dp]),
            "speaker, car, bluetooth"
        );
        assert_eq!(outputs_written(&[Output::Usb; 40]).split(", ").count(), MOST_OUTPUTS);
    }
}
