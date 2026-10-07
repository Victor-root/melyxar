//! Finding out what one device really decodes, rather than what it guesses.
//!
//! The server's half: it cuts short clips out of a real film, in every codec
//! it writes and at every height up to the film's own, rebuilt exactly as a
//! real playback would rebuild them, and keeps them. A browser downloads each
//! clip whole before playing it, so what it measures is its own decoding and
//! nothing else: not how fast this server can produce a picture, and not the
//! network. The earlier calibration produced its film live, and a slow server
//! made a capable browser look incapable.
//!
//! Which film is the most demanding one the library holds. A picture this
//! server generates for itself (see [`melyxar_ffmpeg::calibration`]) was
//! measured to cost a decoder a hundredth of what a real film costs at the
//! same size and rate; it is only what is left for a library nobody has
//! scanned yet.
//!
//! The judgement of each clip is made where it was watched. What is kept here
//! is the result, whole or not at all: a calibration missing a codec the
//! server offered is refused, so a run stopped halfway changes nothing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use melyxar_core::id::PlaybackClientId;
use melyxar_core::time::Millis;
use melyxar_ffmpeg::RunningProcess;

pub use melyxar_database::calibration::{CodecResult, DeviceCalibration, Measurement};

use crate::{AppError, AppState, Result};

/// Which recipe the clips and the measurement of them are made by.
///
/// Carried by every clip folder and every stored calibration, and checked by
/// this server alone: a calibration made by another recipe measured something
/// else and reads as none. The earlier calibration ended at 4.
pub const CALIBRATION_VERSION: i32 = 5;

/// The only codecs a calibration is ever asked about: the ones a picture is
/// ever rebuilt into.
const CALIBRATED_CODECS: &[&str] = &["h264", "hevc", "av1"];

/// The heights a clip is cut at, tallest first, never taller than the film.
const CLIP_HEIGHTS: &[i32] = &[2160, 1440, 1080, 720];

/// One clip a device is to play.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Clip {
    pub codec: String,
    pub height: i32,
    /// What keeping up is counted against.
    pub frame_rate: f64,
}

/// Where the clips stand.
#[derive(Debug, Clone, PartialEq)]
pub enum Readiness {
    /// Nothing has been asked for yet.
    NotPrepared,
    Preparing { done: usize, total: usize },
    /// The last preparation stopped, and why, in the tool's words.
    Failed(String),
    Ready(Vec<Clip>),
}

/// Preparations under way, by the folder they write into.
///
/// Kept in memory: a preparation lives as long as this process, and a clip
/// only counts once it is whole on the disk, so a restart in the middle loses
/// nothing but the clip it was working on.
static PREPARING: LazyLock<Mutex<HashMap<PathBuf, Readiness>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn preparing() -> std::sync::MutexGuard<'static, HashMap<PathBuf, Readiness>> {
    PREPARING.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

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
    /// What the clips cut from this film are kept under: another film chosen
    /// later, or another recipe, is another set of clips.
    key: String,
    source: PathBuf,
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
    who: &melyxar_core::user::User,
    card: Option<&melyxar_ffmpeg::Card>,
) -> Result<MeasuredAgainst> {
    // Among the films this account could have opened itself: it is played to
    // them, whole, for as long as the measure takes.
    let within = crate::reach::within(who);
    if let Some(film) = state
        .database()
        .film_to_measure_against(within.as_deref())
        .await?
    {
        let named = film
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Ok(MeasuredAgainst {
            key: film.source_id.clone(),
            // Its middle rather than its opening: a film opens on logos, on
            // black and on a fade, which is the one passage every machine
            // decodes without trying.
            starts_at: Millis::new(film.duration.get() / 2),
            video_index: film.video_index,
            native_height: film.height,
            frame_rate: film.frame_rate.unwrap_or(FRAME_RATE_WHEN_UNREAD),
            wide_gamut: film.wide_gamut,
            card_reads_it: card.is_some_and(|card| card.reads_for(&film.codec, film.wide_gamut)),
            source: film.path,
            named,
        });
    }

    let generated = reference_film_path(state);
    ensure_reference_film(state, &generated).await?;
    Ok(MeasuredAgainst {
        key: "generated".to_string(),
        source: generated,
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
        .filter(|card| !against.wide_gamut || card.can_tone_map());

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
    Ok(encode)
}

/// The clips one account is measured on, and where they are kept.
struct Plan {
    against: MeasuredAgainst,
    folder: PathBuf,
    clips: Vec<Clip>,
}

impl Plan {
    fn path_of(&self, clip: &Clip) -> PathBuf {
        self.folder
            .join(format!("{}-{}.mp4", clip.codec, clip.height))
    }

    fn missing(&self) -> Vec<&Clip> {
        self.clips
            .iter()
            .filter(|clip| !self.path_of(clip).exists())
            .collect()
    }

    fn codecs(&self) -> Vec<&str> {
        let mut codecs: Vec<&str> = Vec::new();
        for clip in &self.clips {
            if !codecs.contains(&clip.codec.as_str()) {
                codecs.push(&clip.codec);
            }
        }
        codecs
    }
}

/// The heights a film is cut at: every rung of the ladder it is tall enough
/// for, or its own height when it is shorter than all of them.
fn heights_for(native_height: i32) -> Vec<i32> {
    let heights: Vec<i32> = CLIP_HEIGHTS
        .iter()
        .copied()
        .filter(|height| *height <= native_height)
        .collect();
    match heights.is_empty() {
        true => vec![native_height],
        false => heights,
    }
}

/// The codecs a device is asked about: the ones this server is set to write
/// and has something to write them with.
fn codecs_offered(
    allowed: &[String],
    card: Option<&melyxar_ffmpeg::Card>,
    wide_gamut: bool,
) -> Vec<String> {
    CALIBRATED_CODECS
        .iter()
        .filter(|codec| allowed.iter().any(|one| one.eq_ignore_ascii_case(codec)))
        .filter(|codec| {
            let on_the_card = card
                .filter(|card| !wide_gamut || card.can_tone_map())
                .is_some_and(|card| card.encoder_for(codec).is_some());
            on_the_card || codec.eq_ignore_ascii_case(melyxar_playback::profile::ALWAYS_READ)
        })
        .map(|codec| (*codec).to_string())
        .collect()
}

async fn plan(state: &AppState, who: &melyxar_core::user::User) -> Result<Plan> {
    state.capabilities().ok_or_else(no_tools)?;
    let card = crate::cards::in_use(state).await?;
    let against = what_to_measure_against(state, who, card.as_ref()).await?;
    let allowed = state.database().transcoding_limits().await?.video_codecs;
    let codecs = codecs_offered(&allowed, card.as_ref(), against.wide_gamut);
    let heights = heights_for(against.native_height);
    let clips = codecs
        .iter()
        .flat_map(|codec| {
            heights.iter().map(|height| Clip {
                codec: codec.clone(),
                height: *height,
                frame_rate: against.frame_rate,
            })
        })
        .collect();
    let folder = state
        .config()
        .directories
        .calibration()
        .join(format!("clips-v{CALIBRATION_VERSION}-{}", against.key));
    Ok(Plan {
        against,
        folder,
        clips,
    })
}

/// Where the clips one account is measured on stand.
pub async fn readiness(state: &AppState, who: &melyxar_core::user::User) -> Result<Readiness> {
    let plan = plan(state, who).await?;
    if plan.missing().is_empty() {
        return Ok(Readiness::Ready(plan.clips));
    }
    Ok(preparing()
        .get(&plan.folder)
        .cloned()
        .unwrap_or(Readiness::NotPrepared))
}

/// Starts making whatever clips are missing, in the background.
///
/// Asked twice, it makes them once. Each clip is written aside and moved into
/// place whole, so a clip on the disk is always a clip that finished.
pub async fn prepare(state: &AppState, who: &melyxar_core::user::User) -> Result<()> {
    let plan = plan(state, who).await?;
    let missing: Vec<Clip> = plan.missing().into_iter().cloned().collect();
    if missing.is_empty() {
        return Ok(());
    }
    {
        let mut under_way = preparing();
        if matches!(
            under_way.get(&plan.folder),
            Some(Readiness::Preparing { .. })
        ) {
            return Ok(());
        }
        under_way.insert(
            plan.folder.clone(),
            Readiness::Preparing {
                done: 0,
                total: missing.len(),
            },
        );
    }
    let tools = state.tools().ok_or_else(no_tools)?.clone();
    let card = crate::cards::in_use(state).await?;
    tracing::info!(
        film = %plan.against.named,
        clips = missing.len(),
        folder = %plan.folder.display(),
        "the clips a device is measured on are being made"
    );
    tokio::spawn(async move {
        let outcome = make_clips(&tools, card.as_ref(), &plan, &missing).await;
        let mut under_way = preparing();
        match outcome {
            Ok(()) => {
                under_way.remove(&plan.folder);
                tracing::info!("the clips a device is measured on are ready");
            }
            Err(error) => {
                tracing::error!(reason = %error, "the clips a device is measured on could not be made");
                under_way.insert(plan.folder.clone(), Readiness::Failed(error.to_string()));
            }
        }
    });
    Ok(())
}

async fn make_clips(
    tools: &melyxar_ffmpeg::ToolPaths,
    card: Option<&melyxar_ffmpeg::Card>,
    plan: &Plan,
    missing: &[Clip],
) -> Result<()> {
    clear_older_clips(&plan.folder).await;
    tokio::fs::create_dir_all(&plan.folder)
        .await
        .map_err(AppError::Directory)?;
    for (done, clip) in missing.iter().enumerate() {
        let finished = plan.path_of(clip);
        let aside = finished.with_extension("part.mp4");
        let encode = video_encode_for(card, &clip.codec, clip.height, &plan.against)?;
        let command = melyxar_ffmpeg::calibration::clip(
            &plan.against.source,
            plan.against.starts_at,
            plan.against.video_index,
            encode,
            &aside,
        );
        let began = std::time::Instant::now();
        RunningProcess::start(&tools.ffmpeg, &command, None)?
            .wait()
            .await?;
        tokio::fs::rename(&aside, &finished)
            .await
            .map_err(AppError::Directory)?;
        tracing::debug!(
            codec = clip.codec,
            height = clip.height,
            took_ms = began.elapsed().as_millis(),
            "a calibration clip was made"
        );
        preparing().insert(
            plan.folder.clone(),
            Readiness::Preparing {
                done: done + 1,
                total: missing.len(),
            },
        );
    }
    Ok(())
}

/// Removes the clips made by another recipe, which nothing will ask for
/// again.
///
/// Only those: the clips of another film are another account's, measured
/// against what that account can reach, and removing them would only have
/// them made again at its next calibration.
async fn clear_older_clips(beside: &Path) {
    let Some(parent) = beside.parent() else {
        return;
    };
    let Ok(mut entries) = tokio::fs::read_dir(parent).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        if made_by_another_recipe(&entry.file_name().to_string_lossy()) {
            let _ = tokio::fs::remove_dir_all(entry.path()).await;
        }
    }
}

/// Whether a folder of the calibration cache holds clips of another recipe.
fn made_by_another_recipe(folder: &str) -> bool {
    folder.starts_with("clips-")
        && !folder.starts_with(&format!("clips-v{CALIBRATION_VERSION}-"))
}

/// The file of one clip, when it is one this account is measured on and it
/// is ready.
pub async fn clip_file(
    state: &AppState,
    who: &melyxar_core::user::User,
    codec: &str,
    height: i32,
) -> Result<PathBuf> {
    let plan = plan(state, who).await?;
    plan.clips
        .iter()
        .find(|clip| clip.codec.eq_ignore_ascii_case(codec) && clip.height == height)
        .map(|clip| plan.path_of(clip))
        .filter(|path| path.exists())
        .ok_or_else(|| {
            AppError::Domain(melyxar_core::Error::not_found(
                "there is no such calibration clip ready",
            ))
        })
}

/// Whether a calibration answers for exactly the clips that were offered.
///
/// Every codec once, no codec that was not offered, no height that was not
/// one of the clips, and no height measured twice: anything else is a run that
/// did not finish, or one measured against other clips, and neither is a
/// calibration. Measured twice is also how a page would make the kept row,
/// and the journal line about it, as long as it liked.
fn is_whole(offered: &[Clip], codecs: &[CodecResult]) -> bool {
    let mut offered_codecs: Vec<&str> = offered.iter().map(|clip| clip.codec.as_str()).collect();
    offered_codecs.dedup();
    codecs.len() == offered_codecs.len()
        && offered_codecs.iter().all(|codec| {
            codecs.iter().filter(|result| result.codec == *codec).count() == 1
        })
        && codecs.iter().all(|result| {
            let heights_offered = |height: i32| {
                offered
                    .iter()
                    .any(|clip| clip.codec == result.codec && clip.height == height)
            };
            let measured_once = |height: i32| {
                result
                    .measurements
                    .iter()
                    .filter(|measurement| measurement.height == height)
                    .count()
                    == 1
            };
            // Counted first, so a list sent long on purpose is turned away
            // before anything is compared within it.
            let offered_for_it = offered.iter().filter(|clip| clip.codec == result.codec).count();
            result.measurements.len() <= offered_for_it
                && result.smooth_height.is_none_or(heights_offered)
                && result.measurements.iter().all(|measurement| {
                    heights_offered(measurement.height) && measured_once(measurement.height)
                })
        })
}

/// Keeps one device's whole calibration.
pub async fn record(
    state: &AppState,
    who: &melyxar_core::user::User,
    client_id: PlaybackClientId,
    calibration_version: i32,
    codecs: Vec<CodecResult>,
) -> Result<()> {
    if calibration_version != CALIBRATION_VERSION {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "this calibration was made by another recipe",
        )));
    }
    let plan = plan(state, who).await?;
    if !is_whole(&plan.clips, &codecs) {
        tracing::warn!(
            client = %client_id,
            offered = ?plan.codecs(),
            "a calibration that does not answer for every clip offered was refused"
        );
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "this calibration is not whole",
        )));
    }
    for result in &codecs {
        tracing::info!(
            client = %client_id,
            codec = result.codec,
            smooth_height = result.smooth_height,
            measured = ?result.measurements,
            "a device's calibration of one codec"
        );
    }
    state
        .database()
        .save_device_calibration(
            client_id,
            &DeviceCalibration {
                calibration_version,
                measured_at: melyxar_core::time::now(),
                codecs,
            },
        )
        .await?;
    tracing::info!(client = %client_id, "a device's whole calibration was kept");
    Ok(())
}

/// One device's calibration, when it has one made by this recipe.
pub async fn calibration_of(
    state: &AppState,
    client_id: PlaybackClientId,
) -> Result<Option<DeviceCalibration>> {
    Ok(state
        .database()
        .device_calibration(client_id, CALIBRATION_VERSION)
        .await?)
}

/// Forgets one device's calibration.
pub async fn forget(state: &AppState, client_id: PlaybackClientId) -> Result<()> {
    tracing::info!(client = %client_id, "a device's calibration was forgotten");
    Ok(state.database().forget_device_calibration(client_id).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(codec: &str, height: i32) -> Clip {
        Clip {
            codec: codec.to_string(),
            height,
            frame_rate: 24.0,
        }
    }

    fn result(codec: &str, smooth: Option<i32>, heights: &[i32]) -> CodecResult {
        CodecResult {
            codec: codec.to_string(),
            smooth_height: smooth,
            measurements: heights
                .iter()
                .map(|height| Measurement {
                    height: *height,
                    passed: Some(*height) == smooth,
                    dropped_share: 0.0,
                    shown_share: 1.0,
                })
                .collect(),
        }
    }

    fn offered() -> Vec<Clip> {
        vec![
            clip("h264", 1080),
            clip("h264", 720),
            clip("hevc", 1080),
            clip("hevc", 720),
        ]
    }

    #[test]
    fn only_the_clips_of_another_recipe_are_cleared() {
        let current = format!("clips-v{CALIBRATION_VERSION}-another-film");
        assert!(!made_by_another_recipe(&current));
        assert!(made_by_another_recipe("clips-v1-a-film"));
        assert!(!made_by_another_recipe("reference-v2.mkv"));
    }

    #[test]
    fn a_film_is_cut_at_every_rung_it_is_tall_enough_for() {
        assert_eq!(heights_for(2160), vec![2160, 1440, 1080, 720]);
        assert_eq!(heights_for(1600), vec![1440, 1080, 720]);
        assert_eq!(heights_for(576), vec![576]);
    }

    #[test]
    fn a_calibration_answering_for_every_codec_is_whole() {
        assert!(is_whole(
            &offered(),
            &[
                result("h264", Some(1080), &[1080]),
                result("hevc", None, &[1080, 720]),
            ]
        ));
    }

    #[test]
    fn a_run_stopped_before_its_last_codec_is_not_a_calibration() {
        assert!(!is_whole(&offered(), &[result("h264", Some(1080), &[1080])]));
    }

    #[test]
    fn a_codec_or_a_height_that_was_not_offered_is_not_a_calibration() {
        assert!(!is_whole(
            &offered(),
            &[
                result("h264", Some(1080), &[1080]),
                result("av1", Some(1080), &[1080]),
            ]
        ));
        assert!(!is_whole(
            &offered(),
            &[
                result("h264", Some(2160), &[2160]),
                result("hevc", None, &[1080, 720]),
            ]
        ));
        assert!(!is_whole(
            &offered(),
            &[
                result("h264", Some(1080), &[1080]),
                result("h264", Some(1080), &[1080]),
                result("hevc", None, &[1080, 720]),
            ]
        ));
    }

    #[test]
    fn a_height_measured_twice_is_not_a_calibration() {
        assert!(!is_whole(
            &offered(),
            &[
                result("h264", Some(1080), &[1080, 1080]),
                result("hevc", None, &[1080, 720]),
            ]
        ));
        assert!(!is_whole(
            &offered(),
            &[
                result("h264", Some(1080), &[1080]),
                result("hevc", None, &[720; 50_000]),
            ]
        ));
    }

    #[test]
    fn only_a_codec_this_server_can_write_is_offered() {
        let allowed: Vec<String> = ["h264", "hevc", "av1"].map(String::from).to_vec();
        assert_eq!(codecs_offered(&allowed, None, false), vec!["h264"]);
        let only_h264: Vec<String> = vec!["h264".to_string()];
        assert_eq!(codecs_offered(&only_h264, None, true), vec!["h264"]);
    }
}
