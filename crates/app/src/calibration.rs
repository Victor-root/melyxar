//! Finding out what one client really decodes, rather than what it guesses.
//!
//! A session here rebuilds one film into one codec, at one height, so a real
//! browser can be pointed at it and watched. What that browser actually saw
//! is measured on its own side and only ever handed back here to be kept.
//!
//! Which film is the part that took three goes to get right. It was a picture
//! this server generates for itself (see [`melyxar_ffmpeg::calibration`]),
//! made as hard as a generated picture can be made, and it was still not
//! hard enough: measured on real hardware, the same codec at the same size
//! and the same rate lost under one picture in a hundred of it and a third of
//! a real film. What a decoder spends its time on is the coding tools a
//! filmed scene forces an encoder to reach for, not the number of bits, and
//! no fractal asks for those. So the film measured against is the most
//! demanding one the library actually holds, and the generated one is what is
//! left for a library nobody has scanned yet.
//!
//! This module decides nothing about whether a codec was any good: that
//! judgement is made where the picture was watched, against the same
//! dropped-picture signal the rest of playback already reports. What lives
//! here is the part only the server can do: choosing and producing the film,
//! and keeping what came of it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use melyxar_core::id::PlaybackClientId;
use melyxar_core::time::Millis;
use melyxar_ffmpeg::command::{AudioOutput, StreamSelection, VideoOutput};
use melyxar_streaming::session::{Recipe, Session};

pub use melyxar_database::calibration::{CodecCalibration, FoundBy};

use crate::{AppError, AppState, Result};

/// The only codecs a calibration is ever asked about.
///
/// Kept apart from the server's own allow-list on purpose: a codec added to
/// that list later must still be refused here until this module is taught
/// about it, rather than calibrated on a guess about what a fourth codec
/// would even mean for the height ladder below.
const CALIBRATED_CODECS: &[&str] = &["h264", "hevc", "av1"];

/// Where the reference film lives, once it has been made.
///
/// Carries the recipe's own version, so a server that already made one under
/// an older recipe makes a fresh one instead of measuring every client
/// against a film that recipe no longer stands behind.
fn reference_film_path(state: &AppState) -> PathBuf {
    state.config().directories.calibration().join(format!(
        "reference-v{}.mkv",
        melyxar_ffmpeg::calibration::REFERENCE_RECIPE_VERSION
    ))
}

fn no_tools() -> AppError {
    AppError::Domain(melyxar_core::Error::dependency_missing(
        "this server has no media tools, so nothing can be calibrated",
    ))
}

/// Makes the reference film, unless it is already there.
///
/// Made once for the life of the cache, the same way every other picture
/// generated from a film is kept rather than remade on every request.
async fn ensure_reference_film(state: &AppState, path: &Path) -> Result<()> {
    if tokio::fs::try_exists(path).await.unwrap_or(false) {
        return Ok(());
    }
    let tools = state.tools().ok_or_else(no_tools)?;
    tracing::info!(
        seconds = melyxar_ffmpeg::calibration::REFERENCE_DURATION.as_seconds_f64(),
        "making the reference film a calibration is measured against, since none exists yet"
    );
    melyxar_ffmpeg::calibration::make_reference_film(tools, path).await?;
    tracing::info!("the reference film is made and will be kept from now on");
    Ok(())
}

/// What a film is taken to run at when its analysis never read a rate.
///
/// The commonest rate a film is shot at, and the lenient way to be wrong: a
/// film really running faster than this is measured against fewer pictures
/// than it owes, which forgives a decoder rather than condemning one on a
/// number nobody read.
const FRAME_RATE_WHEN_UNREAD: f64 = 24.0;

/// The film a calibration is measured against, once the server has chosen.
struct MeasuredAgainst {
    source: PathBuf,
    /// How long the film runs, which is what its playlist is written from.
    duration: Millis,
    /// The picture's stream inside the file.
    video_index: i32,
    /// Where the measurement begins.
    starts_at: Millis,
    /// The tallest picture this film can offer, which caps what is asked of
    /// it: nothing is invented by asking a film to be taller than it is.
    native_height: i32,
    /// How many pictures a second it runs at.
    frame_rate: f64,
    /// Whether the colour has to be converted on the way out.
    wide_gamut: bool,
    /// Whether the card can read this film for itself, as it would on a real
    /// playback of it.
    card_reads_it: bool,
    /// How it is named in the journal.
    named: String,
}

/// Which film to measure a client against, and where in it.
///
/// A real film whenever the library holds one, because that is the whole
/// point: measured against the film this server generates, the same codec at
/// the same size and the same rate lost under one picture in a hundred where
/// a real film lost a third. The cost of decoding follows the coding tools a
/// filmed scene forces an encoder to reach for, and no generated picture asks
/// for those.
///
/// The generated film is what is left for a library nobody has scanned yet,
/// where there is nothing else to ask.
async fn what_to_measure_against(
    state: &AppState,
    card: Option<&melyxar_ffmpeg::Card>,
) -> Result<MeasuredAgainst> {
    if let Some(film) = state.database().film_to_measure_against().await? {
        let named = film
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Ok(MeasuredAgainst {
            // Its middle rather than its opening: a film opens on logos, on
            // black and on a fade, which is the one passage every machine
            // decodes without trying.
            starts_at: Millis::new(film.duration.get() / 2),
            duration: film.duration,
            video_index: film.video_index,
            native_height: film.height,
            frame_rate: film.frame_rate.unwrap_or(FRAME_RATE_WHEN_UNREAD),
            wide_gamut: film.wide_gamut,
            card_reads_it: card.is_some_and(|card| card.reads(&film.codec)),
            source: film.path,
            named,
        });
    }

    let generated = reference_film_path(state);
    ensure_reference_film(state, &generated).await?;
    Ok(MeasuredAgainst {
        source: generated,
        duration: melyxar_ffmpeg::calibration::REFERENCE_DURATION,
        video_index: 0,
        starts_at: Millis::ZERO,
        native_height: i32::try_from(melyxar_ffmpeg::calibration::REFERENCE_HEIGHT)
            .expect("a picture height fits in a signed integer"),
        frame_rate: f64::from(melyxar_ffmpeg::calibration::REFERENCE_FRAME_RATE),
        wide_gamut: false,
        card_reads_it: card.is_some_and(|card| card.reads("h264")),
        named: "a film this server generated".to_string(),
    })
}

/// What to hand the card, or the processor, to rebuild that film into one
/// codec at one height.
fn video_encode_for(
    card: Option<&melyxar_ffmpeg::Card>,
    codec: &str,
    height: i32,
    against: &MeasuredAgainst,
) -> Result<melyxar_ffmpeg::command::VideoEncode> {
    let usable_card = card
        .filter(|card| card.encoder_for(codec).is_some())
        // A card that cannot convert wide gamut colour would hand back a
        // picture that is grey, which is not what anybody would be measuring.
        // The same rule a real playback of this film goes by.
        .filter(|card| !against.wide_gamut || card.can_tone_map);

    let mut encode = match usable_card {
        Some(card) => {
            melyxar_ffmpeg::command::VideoEncode::on_a_card(card, codec, against.card_reads_it)
                .expect("just proved this card writes this codec")
        }
        None if codec.eq_ignore_ascii_case(melyxar_playback::profile::ALWAYS_READ) => {
            melyxar_ffmpeg::command::VideoEncode::software_h264()
        }
        None => {
            return Err(AppError::Domain(melyxar_core::Error::invalid_input(
                "this server cannot produce that codec from the film it measures against",
            )))
        }
    };
    encode.scale_to_height = (height < against.native_height).then_some(height);
    encode.tone_map = against.wide_gamut;
    encode.max_bitrate = Some(crate::playback::rate_for(Some(height), codec));
    encode.keyframe_interval = Some(melyxar_streaming::playlist::SEGMENT_DURATION);
    Ok(encode)
}

/// Opens a session that rebuilds the film being measured against into one
/// codec, at one height, so a client can be pointed at it and watched.
///
/// Hands back what it really produced beside the session: the height, which
/// is not always the one asked for since a film is never asked to be taller
/// than it is, and the rate, which is what keeping up is counted against on
/// the other side. A client measures against what it really watched.
pub async fn open_calibration_session(
    state: &AppState,
    who: &melyxar_core::user::User,
    codec: &str,
    height: i32,
) -> Result<(Arc<Session>, i32, f64)> {
    if !CALIBRATED_CODECS
        .iter()
        .any(|known| known.eq_ignore_ascii_case(codec))
    {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "this is not a codec a calibration ever asks about",
        )));
    }
    if !state
        .config()
        .transcode
        .enabled_video_codecs
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(codec))
    {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "this server is not configured to transcode into that codec",
        )));
    }

    let sessions = state.sessions().ok_or_else(no_tools)?;
    let capabilities = state.capabilities().ok_or_else(no_tools)?;

    let against = what_to_measure_against(state, capabilities.card()).await?;
    // Never taller than the film itself: asking for more would measure a
    // picture this server would have had to invent.
    let height = height.min(against.native_height);
    let encode = video_encode_for(capabilities.card(), codec, height, &against)?;
    let on_a_card = encode.card().is_some();

    let session = sessions
        .open(
            who.id,
            Recipe {
                source: against.source,
                duration: against.duration,
                streams: StreamSelection {
                    video_index: Some(against.video_index),
                    audio_index: None,
                    subtitle_index: None,
                },
                video: VideoOutput::Encode(encode),
                audio: AudioOutput::None,
                where_the_viewer_starts: against.starts_at,
                where_it_can_be_started: Vec::new(),
                if_the_card_refuses: Vec::new(),
            },
            true,
        )
        .await?;
    tracing::info!(
        session = %session.id,
        codec,
        height,
        at_second = against.starts_at.as_seconds_f64(),
        wide_gamut = against.wide_gamut,
        read_by = if against.card_reads_it { "card" } else { "processor" },
        rebuilt_by = if on_a_card { "card" } else { "processor" },
        measured_against = %against.named,
        "a calibration session was opened"
    );
    Ok((session, height, against.frame_rate))
}

/// Records what one client measured for one codec.
///
/// The one line that answers "did the calibration really run, and what did it
/// conclude": a page saying "optimized" is a claim, and this is where it can
/// be checked against what actually happened, codec by codec.
pub async fn record_calibration(
    state: &AppState,
    client_id: PlaybackClientId,
    calibration: &CodecCalibration,
) -> Result<()> {
    // What a real film did on this machine is not to be talked out of by a
    // test: the test plays a film this server generated, and no generated
    // film costs what a real one costs to decode. Forgetting this device's
    // calibration is what clears a verdict watching established.
    if calibration.found_by == FoundBy::Test {
        let already = state.database().codec_calibrations_of(client_id).await?;
        if already.iter().any(|kept| {
            kept.codec.eq_ignore_ascii_case(&calibration.codec)
                && kept.found_by == FoundBy::Watching
        }) {
            tracing::info!(
                client = %client_id,
                codec = calibration.codec,
                "a test's verdict was left aside: watching a real film already answered for this codec"
            );
            return Ok(());
        }
    }

    tracing::info!(
        client = %client_id,
        codec = calibration.codec,
        usable = calibration.usable,
        tested_height = calibration.tested_height,
        dropped_share = calibration.dropped_share,
        shown_share = calibration.shown_share,
        found_by = calibration.found_by.as_word(),
        calibration_version = calibration.calibration_version,
        "a client's calibration of one codec was recorded"
    );
    state
        .database()
        .save_codec_calibration(client_id, calibration)
        .await?;
    Ok(())
}

/// Everything measured for one client so far, one entry per codec it was
/// asked about.
pub async fn calibration_profile(
    state: &AppState,
    client_id: PlaybackClientId,
) -> Result<Vec<CodecCalibration>> {
    Ok(state.database().codec_calibrations_of(client_id).await?)
}

/// Forgets everything measured for one client, all codecs at once.
pub async fn forget_calibration(state: &AppState, client_id: PlaybackClientId) -> Result<()> {
    tracing::info!(client = %client_id, "a client's whole calibration was forgotten");
    state
        .database()
        .forget_codec_calibrations(client_id)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::{Config, Directories, LibraryConfig, RootConfig};
    use melyxar_database::Database;

    async fn state_without_media_tools() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = Config {
            directories: Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
            },
            libraries: vec![LibraryConfig {
                name: "Films".into(),
                kind: "movies".into(),
                metadata_language: "fr".into(),
                roots: vec![RootConfig {
                    label: "disk-one".into(),
                    path: directory.path().join("films"),
                }],
            }],
            ..Config::default()
        };
        std::fs::create_dir_all(&config.libraries[0].roots[0].path).expect("media folder");
        crate::startup::prepare_directories(&config).expect("directories prepared");

        let database = Database::open_in_memory().await.expect("database opens");
        (directory, AppState::new(config, database, None, None))
    }

    #[tokio::test]
    async fn a_server_with_no_media_tools_cannot_calibrate_anything() {
        let (_directory, state) = state_without_media_tools().await;
        let watching = crate::an_ordinary_account(melyxar_core::id::UserId::new());
        let outcome = open_calibration_session(&state, &watching, "h264", 1080).await;
        assert!(
            outcome.is_err(),
            "nothing can be produced without the tools"
        );
    }

    #[tokio::test]
    async fn a_codec_a_calibration_never_asks_about_is_refused_by_name() {
        let (_directory, state) = state_without_media_tools().await;
        let watching = crate::an_ordinary_account(melyxar_core::id::UserId::new());
        let outcome = open_calibration_session(&state, &watching, "vp9", 1080).await;
        assert!(matches!(outcome, Err(AppError::Domain(_))));
    }

    #[tokio::test]
    async fn nothing_measured_yet_is_an_empty_profile_rather_than_a_fault() {
        let (_directory, state) = state_without_media_tools().await;
        let profile = calibration_profile(&state, PlaybackClientId::new())
            .await
            .expect("read");
        assert!(profile.is_empty());
    }

    #[tokio::test]
    async fn what_is_recorded_is_what_is_read_back() {
        let (_directory, state) = state_without_media_tools().await;
        let client = PlaybackClientId::new();
        let calibration = CodecCalibration {
            codec: "hevc".to_string(),
            calibration_version: 1,
            usable: true,
            tested_height: 2160,
            dropped_share: 0.0,
            shown_share: 1.0,
            found_by: FoundBy::Test,
            measured_at: melyxar_core::time::now(),
        };

        record_calibration(&state, client, &calibration)
            .await
            .expect("recorded");

        let profile = calibration_profile(&state, client).await.expect("read");
        assert_eq!(profile, vec![calibration]);
    }

    #[tokio::test]
    async fn what_a_real_film_proved_is_not_talked_out_of_by_the_test() {
        // The whole reason the two are told apart. The test plays a film this
        // server generated, and no generated film costs what a real one costs
        // to decode: measured on real hardware, the same codec at the same
        // size passed the generated film cleanly and stuttered through every
        // real one. A test must never quietly overwrite that.
        let (_directory, state) = state_without_media_tools().await;
        let client = PlaybackClientId::new();

        let from_a_film = CodecCalibration {
            codec: "av1".to_string(),
            calibration_version: 1,
            usable: false,
            tested_height: 1600,
            dropped_share: 0.31,
            shown_share: 0.69,
            found_by: FoundBy::Watching,
            measured_at: melyxar_core::time::now(),
        };
        record_calibration(&state, client, &from_a_film)
            .await
            .expect("recorded");

        record_calibration(
            &state,
            client,
            &CodecCalibration {
                usable: true,
                dropped_share: 0.0,
                shown_share: 1.0,
                found_by: FoundBy::Test,
                ..from_a_film.clone()
            },
        )
        .await
        .expect("accepted without complaint");

        let profile = calibration_profile(&state, client).await.expect("read");
        assert_eq!(
            profile,
            vec![from_a_film],
            "the film outranks the test, and says so by being the row that is left"
        );
    }

    #[tokio::test]
    async fn a_test_still_answers_for_a_codec_no_film_ever_did() {
        let (_directory, state) = state_without_media_tools().await;
        let client = PlaybackClientId::new();

        record_calibration(
            &state,
            client,
            &CodecCalibration {
                codec: "hevc".to_string(),
                calibration_version: 1,
                usable: false,
                tested_height: 2160,
                dropped_share: 0.4,
                shown_share: 0.6,
                found_by: FoundBy::Watching,
                measured_at: melyxar_core::time::now(),
            },
        )
        .await
        .expect("recorded");

        let av1 = CodecCalibration {
            codec: "av1".to_string(),
            calibration_version: 1,
            usable: true,
            tested_height: 2160,
            dropped_share: 0.0,
            shown_share: 1.0,
            found_by: FoundBy::Test,
            measured_at: melyxar_core::time::now(),
        };
        record_calibration(&state, client, &av1)
            .await
            .expect("recorded");

        let profile = calibration_profile(&state, client).await.expect("read");
        assert!(
            profile.iter().any(|kept| kept == &av1),
            "one codec a film answered for must not silence the test on the others"
        );
    }

    #[tokio::test]
    async fn a_forgotten_calibration_is_offered_afresh() {
        let (_directory, state) = state_without_media_tools().await;
        let client = PlaybackClientId::new();
        record_calibration(
            &state,
            client,
            &CodecCalibration {
                codec: "h264".to_string(),
                calibration_version: 1,
                usable: true,
                tested_height: 1080,
                dropped_share: 0.0,
                shown_share: 1.0,
                found_by: FoundBy::Test,
                measured_at: melyxar_core::time::now(),
            },
        )
        .await
        .expect("recorded");

        forget_calibration(&state, client).await.expect("forgotten");

        assert!(calibration_profile(&state, client)
            .await
            .expect("read")
            .is_empty());
    }
}
