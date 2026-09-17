//! Finding out what one client really decodes, rather than what it guesses.
//!
//! A session here answers no question about a library: it rebuilds the one
//! reference film every installation generates for itself (see
//! [`melyxar_ffmpeg::calibration`]) into one codec, at one height, so a real
//! browser can be pointed at it and watched. What that browser actually saw
//! is measured on its own side and only ever handed back here to be kept.
//!
//! This module decides nothing about whether a codec was any good: that
//! judgement is made where the picture was watched, against the same
//! dropped-picture signal the rest of playback already reports. What lives
//! here is the part only the server can do: producing the film to test with,
//! and keeping what came of it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use melyxar_core::id::PlaybackClientId;
use melyxar_core::time::Millis;
use melyxar_ffmpeg::command::{AudioOutput, StreamSelection, VideoOutput};
use melyxar_streaming::session::{Recipe, Session};

pub use melyxar_database::calibration::CodecCalibration;

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

/// What to hand the card, or the processor, to rebuild the reference film
/// into one codec at one height.
fn video_encode_for(
    card: Option<&melyxar_ffmpeg::Card>,
    codec: &str,
    height: i32,
) -> Result<melyxar_ffmpeg::command::VideoEncode> {
    let mut encode = match card.filter(|card| card.encoder_for(codec).is_some()) {
        Some(card) => melyxar_ffmpeg::command::VideoEncode::on_a_card(card, codec, false)
            .expect("just proved this card writes this codec"),
        None if codec.eq_ignore_ascii_case(melyxar_playback::profile::ALWAYS_READ) => {
            melyxar_ffmpeg::command::VideoEncode::software_h264()
        }
        None => {
            return Err(AppError::Domain(melyxar_core::Error::invalid_input(
                "this server has no card, and only h264 can be produced without one",
            )))
        }
    };
    let native = i32::try_from(melyxar_ffmpeg::calibration::REFERENCE_HEIGHT)
        .expect("a picture height fits in a signed integer");
    encode.scale_to_height = (height < native).then_some(height);
    encode.max_bitrate = Some(crate::playback::rate_for(Some(height), codec));
    encode.keyframe_interval = Some(melyxar_streaming::playlist::SEGMENT_DURATION);
    Ok(encode)
}

/// Opens a session that rebuilds the reference film into one codec, at one
/// height, so a client can be pointed at it and watched.
///
/// Never touches the catalogue: there is no film here for a viewer to have
/// chosen, only a fixed picture this server made for exactly this purpose.
pub async fn open_calibration_session(
    state: &AppState,
    codec: &str,
    height: i32,
) -> Result<Arc<Session>> {
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

    let reference = reference_film_path(state);
    ensure_reference_film(state, &reference).await?;

    let on_a_card = capabilities
        .card()
        .filter(|card| card.encoder_for(codec).is_some())
        .is_some();
    let encode = video_encode_for(capabilities.card(), codec, height)?;

    let session = sessions
        .open(
            Recipe {
                source: reference,
                duration: melyxar_ffmpeg::calibration::REFERENCE_DURATION,
                streams: StreamSelection {
                    video_index: Some(0),
                    audio_index: None,
                    subtitle_index: None,
                },
                video: VideoOutput::Encode(encode),
                audio: AudioOutput::None,
                where_the_viewer_starts: Millis::ZERO,
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
        rebuilt_by = if on_a_card { "card" } else { "processor" },
        "a calibration session was opened against the reference film"
    );
    Ok(session)
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
    tracing::info!(
        client = %client_id,
        codec = calibration.codec,
        usable = calibration.usable,
        tested_height = calibration.tested_height,
        dropped_share = calibration.dropped_share,
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
    state.database().forget_codec_calibrations(client_id).await?;
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
        let outcome = open_calibration_session(&state, "h264", 1080).await;
        assert!(outcome.is_err(), "nothing can be produced without the tools");
    }

    #[tokio::test]
    async fn a_codec_a_calibration_never_asks_about_is_refused_by_name() {
        let (_directory, state) = state_without_media_tools().await;
        let outcome = open_calibration_session(&state, "vp9", 1080).await;
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
            measured_at: melyxar_core::time::now(),
        };

        record_calibration(&state, client, &calibration)
            .await
            .expect("recorded");

        let profile = calibration_profile(&state, client).await.expect("read");
        assert_eq!(profile, vec![calibration]);
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
                measured_at: melyxar_core::time::now(),
            },
        )
        .await
        .expect("recorded");

        forget_calibration(&state, client).await.expect("forgotten");

        assert!(calibration_profile(&state, client).await.expect("read").is_empty());
    }
}
