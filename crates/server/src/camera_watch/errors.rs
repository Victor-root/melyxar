//! Everything that failed around a camera, under the `camera-error` tag,
//! with the chain of its causes, and a crash of the application while a
//! camera was open.

use serde::Deserialize;

use super::Said;
use crate::page::cut_to;

/// How much of an error is written down.
const ENOUGH_OF_AN_ERROR: usize = 600;
const ENOUGH_OF_A_CODE: usize = 80;

/// Where it failed.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum During {
    /// Asking the server where the camera is.
    Server,
    /// The exchange with the camera.
    Rtsp,
    Player,
    Load,
    VideoCodec,
    AudioCodec,
    AudioSink,
    /// The application crashed, a camera open.
    Crash,
}

impl During {
    fn as_word(self) -> &'static str {
        match self {
            Self::Server => "asking the server where the camera is",
            Self::Rtsp => "the exchange with the camera",
            Self::Player => "the player",
            Self::Load => "receiving from the camera",
            Self::VideoCodec => "decoding the picture",
            Self::AudioCodec => "decoding the sound",
            Self::AudioSink => "playing the sound",
            Self::Crash => "the application crashed",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct Failed {
    during: During,
    #[serde(default)]
    code: Option<String>,
    causes: String,
}

/// The words of an error with the account taken out of every address in
/// them, `rtsp://name:password@camera` written `rtsp://***@camera`. The
/// application already takes it out; this is the server not trusting that.
fn without_accounts(words: &str) -> String {
    let mut out = String::with_capacity(words.len());
    let mut rest = words;
    while let Some(at) = rest.find("://") {
        let (before, after) = rest.split_at(at + 3);
        out.push_str(before);
        let host_ends = after
            .find(|c: char| c == '/' || c.is_whitespace())
            .unwrap_or(after.len());
        match after[..host_ends].rfind('@') {
            Some(account_ends) => {
                out.push_str("***");
                rest = &after[account_ends..];
            }
            None => rest = after,
        }
    }
    out.push_str(rest);
    out
}

pub(super) fn write(by: &Said, seen: &Failed) {
    tracing::warn!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        code = seen.code.as_deref().map(|code| without_accounts(cut_to(code, ENOUGH_OF_A_CODE))),
        causes = without_accounts(cut_to(&seen.causes, ENOUGH_OF_AN_ERROR)),
        "failed: {}",
        seen.during.as_word()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_account_is_written_from_an_address() {
        assert_eq!(
            without_accounts("could not reach rtsp://admin:s3cret@10.0.0.9:554/stream1 after 3 tries"),
            "could not reach rtsp://***@10.0.0.9:554/stream1 after 3 tries"
        );
        assert_eq!(
            without_accounts("rtsps://a:b@one/x and rtsp://c:d@two"),
            "rtsps://***@one/x and rtsp://***@two"
        );
        assert_eq!(without_accounts("rtsp://10.0.0.9/stream1 timed out"), "rtsp://10.0.0.9/stream1 timed out");
        assert_eq!(without_accounts("nothing to hide"), "nothing to hide");
    }
}
