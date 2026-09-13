//! Answering "how does this film reach me, and where was I?".
//!
//! The decision itself is a pure rule living in its own crate, which knows
//! nothing of files or databases. This is where it meets them: the file is
//! read back with its tracks, the rule is asked, and the answer is handed over
//! with what a player needs to start.
//!
//! Nothing here opens the file. Serving bytes belongs to the HTTP layer, which
//! is the only part that knows what a range request is.

use std::path::PathBuf;
use std::sync::Arc;

use melyxar_core::id::{MediaSourceId, TrackId, UserId, WorkId};
use melyxar_core::media::Track;
use melyxar_core::time::{Millis, Timestamp};
use melyxar_core::work::{state_for_position, PlaybackState, DEFAULT_WATCHED_THRESHOLD};
use melyxar_playback::decision::{decide, PlaybackDecision, PlaybackRequest};

use crate::{AppError, AppState, Result};

/// What a client says it can open, and the shape of the answer.
///
/// Re-exported here so the layer above talks to one crate: the decision lives
/// where it can be tested without a database, and nothing outside has to know
/// that it does.
pub use melyxar_playback::decision::{PlaybackMethod, Reason, StreamAction, SubtitleDelivery};
pub use melyxar_playback::profile::ClientProfile;

/// A film being converted as it is watched, as the layer above handles it.
///
/// Re-exported for the same reason as the decision: the HTTP layer talks to
/// one crate, and where a session really lives stays this crate's business.
pub use melyxar_streaming::session::{Recipe, Session};
pub use melyxar_streaming::StreamingError;

/// What a viewer asked to play, in the words of a client.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayRequest {
    pub source_id: MediaSourceId,
    /// What the client says it can open. Absent means a cautious browser,
    /// which is what a client that says nothing turns out to be.
    pub profile: Option<ClientProfile>,
    /// Chosen soundtrack. Absent means the one the file marks as default.
    pub audio_track_id: Option<TrackId>,
    /// Chosen subtitle. Absent means none.
    pub subtitle_track_id: Option<TrackId>,
}

/// Everything a player needs to start.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayPlan {
    pub source_id: MediaSourceId,
    pub work_id: WorkId,
    /// Where the file is. Kept for the layer that serves the bytes and never
    /// sent to a client.
    pub path: PathBuf,
    pub size_bytes: i64,
    pub duration: Option<Millis>,
    pub decision: PlaybackDecision,
    /// Where this viewer stopped last time, when they did.
    pub resume_from: Option<Millis>,
    pub tracks: Vec<Track>,
}

/// Works out how one file reaches one client.
pub async fn plan(state: &AppState, user_id: UserId, request: &PlayRequest) -> Result<PlayPlan> {
    let database = state.database();

    let source = database
        .playable_source(request.source_id)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("media source")))?;

    if source.missing {
        // Refused with an explanation rather than opened and failing halfway
        // through, which is what a viewer would otherwise see.
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::RootUnavailable,
            "the file is not on the disk at the moment",
        )));
    }

    let tracks = database.tracks_of_source(request.source_id).await?;
    let media = melyxar_core::media::MediaSource {
        id: source.id,
        work_id: source.work_id,
        root_id: melyxar_core::id::LibraryRootId::new(),
        relative_path: source.path.clone(),
        container: source.container.clone(),
        duration: source.duration,
        overall_bitrate: None,
        identity: melyxar_core::media::FileIdentity {
            size_bytes: source.size_bytes,
            modified_at: melyxar_core::time::now(),
            content_fingerprint: None,
        },
        added_at: melyxar_core::time::now(),
        tracks: tracks.clone(),
    };

    let profile = request
        .profile
        .clone()
        .unwrap_or_else(ClientProfile::conservative_browser);

    let preferences = database.user(user_id).await?.map(|user| user.preferences);
    let remembered = database.playback_progress(user_id, source.work_id).await?;

    // What the viewer asked for now, otherwise what they chose last time for
    // this very film, otherwise a track in the language they prefer. Only then
    // does the file get to decide, which is what happens today for everyone.
    let audio = chosen_track(
        &tracks,
        request.audio_track_id,
        remembered.as_ref().and_then(|stored| stored.audio_track_id),
        preferences
            .as_ref()
            .and_then(|values| values.preferred_audio_language.as_deref()),
        TrackShape::Audio,
    );
    let subtitle = match request.subtitle_track_id {
        // A subtitle is only shown when someone asks for one: a film that
        // starts with subtitles nobody wanted is a film someone stops.
        Some(id) => tracks.iter().find(|track| track.id == id),
        None => chosen_track(
            &tracks,
            None,
            remembered
                .as_ref()
                .and_then(|stored| stored.subtitle_track_id),
            preferences
                .as_ref()
                .and_then(|values| values.preferred_subtitle_language.as_deref()),
            TrackShape::Subtitle,
        ),
    };
    let decision = decide(
        &media,
        &PlaybackRequest {
            profile: &profile,
            audio_track: audio,
            subtitle_track: subtitle,
            downmix: preferences
                .as_ref()
                .map(|values| values.downmix_method)
                .unwrap_or_default(),
            // Nothing asks for levelling yet: no preference carries it, and
            // asking for it without a measurement to level by would turn a
            // copy into a rebuild for nothing. When the preference arrives it
            // goes here, and the answer will say it was asked for.
            level_loudness: false,
        },
    );

    let resume_from = remembered
        .filter(|progress| progress.state == PlaybackState::InProgress)
        .map(|progress| progress.position);

    Ok(PlayPlan {
        source_id: source.id,
        work_id: source.work_id,
        path: source.path,
        size_bytes: source.size_bytes,
        duration: source.duration,
        decision,
        resume_from,
        tracks,
    })
}

/// Which of the two kinds of track is being looked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrackShape {
    Audio,
    Subtitle,
}

impl TrackShape {
    fn matches(self, track: &Track) -> bool {
        match self {
            Self::Audio => matches!(track.kind, melyxar_core::media::TrackKind::Audio(_)),
            Self::Subtitle => matches!(track.kind, melyxar_core::media::TrackKind::Subtitle(_)),
        }
    }
}

/// Picks a track, in the order a viewer would expect.
///
/// What they asked for now beats what they chose last time, which beats the
/// language they prefer in general. A choice pointing at a track the file no
/// longer holds falls through to the next rule rather than to nothing: an
/// analysis run again renumbers the tracks, and a viewer should not have to
/// notice.
fn chosen_track<'a>(
    tracks: &'a [Track],
    asked_for: Option<TrackId>,
    remembered: Option<TrackId>,
    preferred_language: Option<&str>,
    shape: TrackShape,
) -> Option<&'a Track> {
    let by_id = |wanted: TrackId| tracks.iter().find(|track| track.id == wanted);

    if let Some(track) = asked_for.and_then(by_id) {
        return Some(track);
    }
    if let Some(track) = remembered.and_then(by_id) {
        return Some(track);
    }

    let wanted = preferred_language.map(melyxar_core::media::normalise_language)?;
    tracks.iter().find(|track| {
        shape.matches(track)
            && track
                .language
                .as_deref()
                .map(melyxar_core::media::normalise_language)
                .is_some_and(|spoken| spoken == wanted)
    })
}

/// Remembers the tracks a viewer chose, and the languages they were in.
///
/// Both, and for different reasons. The tracks so this film starts the same
/// way next time, down to the track; the languages so the next film does too,
/// even though its tracks are numbered differently.
pub async fn remember_chosen_tracks(
    state: &AppState,
    user_id: UserId,
    work_id: WorkId,
    audio: Option<&Track>,
    subtitle: Option<&Track>,
) -> Result<()> {
    let database = state.database();
    database
        .record_chosen_tracks(
            user_id,
            work_id,
            audio.map(|track| track.id),
            subtitle.map(|track| track.id),
        )
        .await?;

    if let Some(user) = database.user(user_id).await? {
        let mut preferences = user.preferences;
        // A track with no language declared teaches nothing about what this
        // viewer prefers, so it leaves the preference alone.
        if let Some(language) = audio.and_then(|track| track.language.clone()) {
            preferences.preferred_audio_language = Some(language);
        }
        // Subtitles are different: turning them off is itself a preference,
        // and one a viewer expects to hold for the next film too.
        preferences.preferred_subtitle_language = subtitle.and_then(|track| track.language.clone());
        database.save_preferences(user_id, &preferences).await?;
    }
    Ok(())
}

/// Opens a session that produces this film in a form the client can play.
///
/// Everything about what to produce was already decided; this turns that
/// decision into the recipe a session carries out. A film the client could
/// play as it is never reaches here: it is served as a file, which costs
/// nothing at all.
pub async fn open_session(state: &AppState, plan: &PlayPlan) -> Result<Arc<Session>> {
    let sessions = state.sessions().ok_or_else(|| {
        AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::DependencyMissing,
            "this server has no media tools, so nothing can be converted",
        ))
    })?;
    let capabilities = state.capabilities().ok_or_else(|| {
        AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::DependencyMissing,
            "this server has no media tools, so nothing can be converted",
        ))
    })?;

    let recipe = recipe_for(plan, capabilities)?;
    let expensive = plan.decision.method.is_expensive();
    Ok(sessions.open(recipe, expensive).await?)
}

/// Turns a decision into what the tool is asked to do.
fn recipe_for(plan: &PlayPlan, capabilities: &melyxar_ffmpeg::Capabilities) -> Result<Recipe> {
    use melyxar_ffmpeg::command::{AudioOutput, StreamSelection, VideoOutput};
    use melyxar_playback::decision::StreamAction;

    let duration = plan.duration.ok_or_else(|| {
        // Without a duration there is no playlist to write: a film nobody has
        // looked inside cannot be cut into segments.
        AppError::Domain(melyxar_core::Error::invalid_input(
            "this file has not been analysed yet",
        ))
    })?;

    let video = match plan.decision.video {
        StreamAction::Drop => VideoOutput::None,
        StreamAction::Copy => VideoOutput::Copy,
        StreamAction::Transcode => {
            let mut encode = melyxar_ffmpeg::command::VideoEncode::software_h264();
            encode.scale_to_height = plan.decision.scale_to_height;
            encode.tone_map = plan.decision.tone_map;
            // Key frames on the segment boundaries, which is what lets any
            // segment be produced on its own rather than only after the one
            // before it.
            encode.keyframe_interval = Some(melyxar_streaming::playlist::SEGMENT_DURATION);
            VideoOutput::Encode(encode)
        }
    };

    let audio = match plan.decision.audio {
        StreamAction::Drop => AudioOutput::None,
        StreamAction::Copy => AudioOutput::Copy,
        StreamAction::Transcode => {
            let encoder = capabilities.audio_encoder().ok_or_else(|| {
                AppError::Domain(melyxar_core::Error::new(
                    melyxar_core::error::ErrorCode::DependencyMissing,
                    "these media tools cannot build a soundtrack a browser reads",
                ))
            })?;
            AudioOutput::Encode(melyxar_ffmpeg::command::AudioEncode::browser_stereo(
                encoder,
            ))
        }
    };

    Ok(Recipe {
        source: plan.path.clone(),
        duration,
        streams: StreamSelection {
            video_index: plan.decision.video_stream_index,
            audio_index: plan.decision.audio_stream_index,
            subtitle_index: None,
        },
        video,
        audio,
    })
}

/// Records where a viewer got to.
///
/// Answers whether the report was kept: one arriving after a fresher one is
/// refused, so a client that reconnects cannot make the resume point go
/// backwards.
pub async fn record_position(
    state: &AppState,
    user_id: UserId,
    work_id: WorkId,
    position: Millis,
    reported_at: Timestamp,
) -> Result<bool> {
    let database = state.database();
    let duration = longest_version(state, work_id).await?;

    let marked_manually = database
        .playback_progress(user_id, work_id)
        .await?
        .is_some_and(|progress| progress.marked_manually);

    let state_now = state_for_position(
        position,
        duration,
        DEFAULT_WATCHED_THRESHOLD,
        marked_manually,
    );

    Ok(database
        .record_playback_progress(user_id, work_id, position, state_now, reported_at)
        .await?)
}

/// How long the work runs, from the longest copy of it that was analysed.
///
/// The runtime a provider gave describes the film; what decides whether
/// someone reached the end is the file they are actually watching.
async fn longest_version(state: &AppState, work_id: WorkId) -> Result<Option<Millis>> {
    let database = state.database();
    let mut longest = None;
    for source in database.sources_of_work(work_id).await? {
        if let Some((analysis, _)) = database.source_details(source.id).await? {
            longest = match (longest, analysis.duration) {
                (Some(known), Some(found)) if found > known => Some(found),
                (None, found) => found,
                (known, _) => known,
            };
        }
    }
    Ok(longest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::id::MediaSourceId;
    use melyxar_core::media::{
        AudioDetails, ColorInfo, Loudness, SubtitleDetails, SubtitleLayout, TrackKind, VideoDetails,
    };
    use melyxar_core::user::Permissions;
    use melyxar_database::catalogue::SourceAnalysis;
    use melyxar_database::Database;
    use melyxar_playback::decision::{PlaybackMethod, SubtitleDelivery};
    use time::macros::datetime;

    fn video(source_id: MediaSourceId, codec: &str, height: i32) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 0,
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
                aspect_ratio: None,
                is_interlaced: false,
                frame_rate: Some(24.0),
                bitrate: None,
                pixel_format: None,
                reference_frames: None,
                color: ColorInfo::default(),
                hdr: None,
            }),
        }
    }

    fn audio(source_id: MediaSourceId, codec: &str, channels: i32, is_default: bool) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 1,
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

    fn subtitle(source_id: MediaSourceId) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 2,
            language: Some("fre".into()),
            title: None,
            is_default: false,
            is_forced: false,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: "subrip".into(),
                layout: SubtitleLayout::Text,
                is_hearing_impaired: false,
                is_external: false,
                external_relative_path: None,
            }),
        }
    }

    /// One film in one library, with the file named as the client will ask
    /// for it and the tracks a browser has to judge.
    async fn state_with_film(
        file_name: &str,
        container: &str,
        tracks: impl FnOnce(MediaSourceId) -> Vec<Track>,
    ) -> (tempfile::TempDir, AppState, UserId, MediaSourceId) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        std::fs::create_dir_all(&media).expect("media folder");
        std::fs::write(media.join(file_name), b"not a real film").expect("file written");

        let config = melyxar_config::Config {
            directories: melyxar_config::Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
            },
            libraries: vec![melyxar_config::LibraryConfig {
                name: "Films".into(),
                kind: "movies".into(),
                metadata_language: "fr".into(),
                roots: vec![melyxar_config::RootConfig {
                    label: "disk-one".into(),
                    path: media,
                }],
            }],
            ..melyxar_config::Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");

        let database = Database::open_in_memory().await.expect("database opens");
        crate::startup::reconcile_libraries(&database, &config)
            .await
            .expect("libraries reconciled");
        let library = database
            .library_by_name("Films")
            .await
            .expect("read")
            .expect("declared");
        let work = database
            .create_work(
                library.id,
                melyxar_core::work::WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        let source_id = database
            .insert_source(
                work.id,
                library.roots[0].id,
                std::path::Path::new(file_name),
                12_000,
                melyxar_core::time::now(),
            )
            .await
            .expect("source recorded");
        database
            .store_analysis(
                source_id,
                &SourceAnalysis {
                    container: Some(container.to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: None,
                },
                &tracks(source_id),
                &[],
            )
            .await
            .expect("analysis stored");

        let user = database
            .create_user("victor", None, &Permissions::administrator())
            .await
            .expect("account created");
        let state = AppState::new(config, database, None, None);
        (directory, state, user.id, source_id)
    }

    #[tokio::test]
    async fn a_film_a_browser_can_open_is_handed_over_untouched() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![video(id, "h264", 1080), audio(id, "aac", 2, true)]
            })
            .await;

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");

        assert_eq!(plan.decision.method, PlaybackMethod::DirectPlay);
        assert!(plan.path.ends_with("Quiet.Harbour.2019.mp4"));
        assert_eq!(plan.size_bytes, 12_000);
        assert_eq!(plan.duration, Some(Millis::new(7_200_000)));
        assert!(plan.resume_from.is_none(), "nobody has watched it yet");
    }

    #[tokio::test]
    async fn a_soundtrack_no_browser_decodes_is_rebuilt_and_the_picture_is_not() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mkv", "matroska,webm", |id| {
                vec![video(id, "h264", 1080), audio(id, "eac3", 6, true)]
            })
            .await;

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");

        assert_eq!(
            plan.decision.method,
            PlaybackMethod::TranscodeAudio,
            "the common case here: the picture is fine and the sound is not"
        );
        assert_eq!(
            plan.decision.video,
            melyxar_playback::decision::StreamAction::Copy,
            "rebuilding a picture that plays perfectly well costs an hour of a graphics card"
        );
        assert!(!plan.decision.reasons.is_empty(), "and it says why");
    }

    #[tokio::test]
    async fn a_chosen_subtitle_travels_beside_the_picture() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![
                    video(id, "h264", 1080),
                    audio(id, "aac", 2, true),
                    subtitle(id),
                ]
            })
            .await;

        let tracks = state
            .database()
            .tracks_of_source(source_id)
            .await
            .expect("read");
        let chosen = tracks
            .iter()
            .find(|track| matches!(track.kind, TrackKind::Subtitle(_)))
            .expect("a subtitle");

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: Some(chosen.id),
            },
        )
        .await
        .expect("a plan");

        assert_eq!(plan.decision.subtitles, SubtitleDelivery::External);
        assert_eq!(plan.decision.subtitle_stream_index, Some(2));
    }

    /// Two soundtracks, French second, so the file's own order and the
    /// viewer's preference disagree.
    fn two_soundtracks(source_id: MediaSourceId) -> Vec<Track> {
        let mut english = audio(source_id, "aac", 2, true);
        english.language = Some("eng".to_string());
        english.stream_index = 1;
        let mut french = audio(source_id, "aac", 2, false);
        french.language = Some("fre".to_string());
        french.stream_index = 2;
        vec![video(source_id, "h264", 1080), english, french]
    }

    async fn soundtrack_of(
        state: &AppState,
        user: UserId,
        source: MediaSourceId,
    ) -> Option<String> {
        let plan = plan(
            state,
            user,
            &PlayRequest {
                source_id: source,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");
        let index = plan.decision.audio_stream_index?;
        plan.tracks
            .iter()
            .find(|track| track.stream_index == index)
            .and_then(|track| track.language.clone())
    }

    #[tokio::test]
    async fn without_a_preference_the_file_decides_which_soundtrack_plays() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", two_soundtracks).await;
        assert_eq!(
            soundtrack_of(&state, user_id, source_id).await.as_deref(),
            Some("eng"),
            "the one the file marks as default"
        );
    }

    #[tokio::test]
    async fn the_language_a_viewer_prefers_is_picked_without_being_asked_again() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", two_soundtracks).await;

        let tracks = state
            .database()
            .tracks_of_source(source_id)
            .await
            .expect("read");
        let french = tracks
            .iter()
            .find(|track| track.language.as_deref() == Some("fre"))
            .expect("a French soundtrack");
        let work_id = state
            .database()
            .playable_source(source_id)
            .await
            .expect("read")
            .expect("present")
            .work_id;

        remember_chosen_tracks(&state, user_id, work_id, Some(french), None)
            .await
            .expect("choice remembered");

        assert_eq!(
            soundtrack_of(&state, user_id, source_id).await.as_deref(),
            Some("fre"),
            "the same film starts the way it was left"
        );

        // Another film altogether, whose tracks are numbered differently: the
        // language is what carries over, not the identifier.
        let (_elsewhere, other_state, other_user, other_source) =
            state_with_film("Amber.Field.2020.mp4", "mov,mp4,m4a", two_soundtracks).await;
        let mut preferences = other_state
            .database()
            .user(other_user)
            .await
            .expect("read")
            .expect("present")
            .preferences;
        preferences.preferred_audio_language = Some("fr".to_string());
        other_state
            .database()
            .save_preferences(other_user, &preferences)
            .await
            .expect("preferences saved");

        assert_eq!(
            soundtrack_of(&other_state, other_user, other_source)
                .await
                .as_deref(),
            Some("fre"),
            "written as two letters or three, it is the same language"
        );
    }

    #[tokio::test]
    async fn what_was_chosen_for_this_film_beats_the_language_preferred_in_general() {
        // A viewer who watches everything in French but this one film in its
        // own language gets that film in its own language.
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", two_soundtracks).await;
        let tracks = state
            .database()
            .tracks_of_source(source_id)
            .await
            .expect("read");
        let english = tracks
            .iter()
            .find(|track| track.language.as_deref() == Some("eng"))
            .expect("an English soundtrack");
        let work_id = state
            .database()
            .playable_source(source_id)
            .await
            .expect("read")
            .expect("present")
            .work_id;

        state
            .database()
            .record_chosen_tracks(user_id, work_id, Some(english.id), None)
            .await
            .expect("choice recorded");
        let mut preferences = state
            .database()
            .user(user_id)
            .await
            .expect("read")
            .expect("present")
            .preferences;
        preferences.preferred_audio_language = Some("fre".to_string());
        state
            .database()
            .save_preferences(user_id, &preferences)
            .await
            .expect("preferences saved");

        assert_eq!(
            soundtrack_of(&state, user_id, source_id).await.as_deref(),
            Some("eng"),
            "the choice made about this film is more particular than a habit"
        );
    }

    #[tokio::test]
    async fn a_film_never_starts_with_subtitles_nobody_asked_for() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![
                    video(id, "h264", 1080),
                    audio(id, "aac", 2, true),
                    subtitle(id),
                ]
            })
            .await;

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");
        assert_eq!(plan.decision.subtitles, SubtitleDelivery::None);
        assert_eq!(plan.decision.subtitle_stream_index, None);
    }

    #[tokio::test]
    async fn a_fold_to_stereo_someone_asked_for_is_a_reason_of_its_own() {
        // The preference has to reach the decision, or it quietly does nothing
        // on every film a client could have played as it is. The client here
        // accepts six channels, so nothing else explains the rebuild.
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![video(id, "h264", 1080), audio(id, "aac", 6, true)]
            })
            .await;

        let mut profile = ClientProfile::conservative_browser();
        profile.max_audio_channels = Some(6);
        let request = PlayRequest {
            source_id,
            profile: Some(profile),
            audio_track_id: None,
            subtitle_track_id: None,
        };

        // A server nobody has configured already folds the sound its own way
        // rather than leaving it to the browser, whose fold buries the
        // dialogue under the effects. That costs a rebuild of the sound, and
        // the answer says as much.
        let out_of_the_box = plan(&state, user_id, &request).await.expect("a plan");
        assert_eq!(
            out_of_the_box.decision.method,
            PlaybackMethod::TranscodeAudio
        );

        let mut preferences = state
            .database()
            .user(user_id)
            .await
            .expect("read")
            .expect("present")
            .preferences;
        preferences.downmix_method = melyxar_core::user::DownmixMethod::None;
        state
            .database()
            .save_preferences(user_id, &preferences)
            .await
            .expect("preferences saved");
        assert_eq!(
            plan(&state, user_id, &request)
                .await
                .expect("a plan")
                .decision
                .method,
            PlaybackMethod::DirectPlay,
            "a viewer who leaves the fold to the browser gets the file untouched"
        );

        preferences.downmix_method = melyxar_core::user::DownmixMethod::NightDialogue;
        state
            .database()
            .save_preferences(user_id, &preferences)
            .await
            .expect("preferences saved");

        let folded = plan(&state, user_id, &request).await.expect("a plan");
        assert_eq!(
            folded.decision.method,
            PlaybackMethod::TranscodeAudio,
            "a way of folding to stereo can only be applied by rebuilding the sound"
        );
        assert!(
            folded.decision.reasons.iter().any(|reason| matches!(
                reason,
                melyxar_playback::decision::Reason::DownmixRequested { .. }
            )),
            "and the answer says it was asked for: {:?}",
            folded.decision.reasons
        );
    }

    fn capabilities_of_a_usual_tool() -> melyxar_ffmpeg::Capabilities {
        melyxar_ffmpeg::Capabilities {
            version: "ffmpeg version invented".to_string(),
            encoders: ["libx264", "aac"].iter().map(|v| v.to_string()).collect(),
            decoders: Default::default(),
            filters: Default::default(),
            hardware: Default::default(),
        }
    }

    #[tokio::test]
    async fn a_rebuilt_picture_is_cut_where_the_segments_are() {
        // Without key frames on the boundaries, a segment can only be produced
        // after the one before it, and jumping stops working: the whole reason
        // the server owns the playlist would be lost.
        use melyxar_ffmpeg::command::VideoOutput;

        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mkv", "matroska,webm", |id| {
                vec![video(id, "hevc", 2160), audio(id, "eac3", 6, true)]
            })
            .await;

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");
        assert_eq!(plan.decision.method, PlaybackMethod::FullTranscode);

        let recipe = recipe_for(&plan, &capabilities_of_a_usual_tool()).expect("a recipe");
        let VideoOutput::Encode(encode) = recipe.video else {
            panic!("a picture no browser reads is rebuilt");
        };
        assert_eq!(
            encode.keyframe_interval,
            Some(melyxar_streaming::playlist::SEGMENT_DURATION),
            "a key frame on every segment boundary"
        );
        assert!(matches!(
            recipe.audio,
            melyxar_ffmpeg::command::AudioOutput::Encode(_)
        ));
    }

    #[tokio::test]
    async fn a_picture_a_browser_reads_is_copied_and_only_the_sound_is_rebuilt() {
        use melyxar_ffmpeg::command::{AudioOutput, VideoOutput};

        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mkv", "matroska,webm", |id| {
                vec![video(id, "h264", 1080), audio(id, "eac3", 6, true)]
            })
            .await;

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");

        let recipe = recipe_for(&plan, &capabilities_of_a_usual_tool()).expect("a recipe");
        assert_eq!(
            recipe.video,
            VideoOutput::Copy,
            "rebuilding a picture that plays perfectly well costs an hour of a machine"
        );
        assert!(matches!(recipe.audio, AudioOutput::Encode(_)));
        assert_eq!(recipe.duration, Millis::new(7_200_000));
    }

    #[tokio::test]
    async fn a_film_nobody_has_looked_inside_cannot_be_cut_into_segments() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mkv", "matroska,webm", |id| {
                vec![video(id, "h264", 1080), audio(id, "eac3", 6, true)]
            })
            .await;

        let mut plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");
        plan.duration = None;

        assert!(
            recipe_for(&plan, &capabilities_of_a_usual_tool()).is_err(),
            "without a length there is no playlist to write, and a player would \
             be handed a film of no duration"
        );
    }

    #[tokio::test]
    async fn a_file_that_is_not_on_the_disk_is_refused_rather_than_opened() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![video(id, "h264", 1080), audio(id, "aac", 2, true)]
            })
            .await;
        state
            .database()
            .mark_source_missing(source_id)
            .await
            .expect("marked");

        let outcome = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await;
        assert!(
            outcome.is_err(),
            "a viewer is told before pressing play, not halfway through"
        );
    }

    #[tokio::test]
    async fn a_film_someone_left_halfway_offers_to_carry_on() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![video(id, "h264", 1080), audio(id, "aac", 2, true)]
            })
            .await;
        let work_id = state
            .database()
            .playable_source(source_id)
            .await
            .expect("read")
            .expect("present")
            .work_id;

        assert!(record_position(
            &state,
            user_id,
            work_id,
            Millis::new(1_800_000),
            datetime!(2026-01-01 12:00 UTC),
        )
        .await
        .expect("recorded"));

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");
        assert_eq!(plan.resume_from, Some(Millis::new(1_800_000)));
    }

    #[tokio::test]
    async fn a_film_watched_to_the_end_starts_again_from_the_beginning() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![video(id, "h264", 1080), audio(id, "aac", 2, true)]
            })
            .await;
        let work_id = state
            .database()
            .playable_source(source_id)
            .await
            .expect("read")
            .expect("present")
            .work_id;

        // Two hours in, of a two hour film.
        record_position(
            &state,
            user_id,
            work_id,
            Millis::new(7_100_000),
            datetime!(2026-01-01 12:00 UTC),
        )
        .await
        .expect("recorded");

        assert_eq!(
            state
                .database()
                .playback_progress(user_id, work_id)
                .await
                .expect("read")
                .expect("present")
                .state,
            PlaybackState::Watched
        );

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");
        assert!(
            plan.resume_from.is_none(),
            "offering to carry on ten seconds before the credits helps nobody"
        );
    }

    #[tokio::test]
    async fn where_someone_is_counts_against_the_copy_they_are_watching() {
        // The runtime a provider gave describes the film; the file is what
        // decides whether someone reached the end of it.
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![video(id, "h264", 1080), audio(id, "aac", 2, true)]
            })
            .await;
        let work_id = state
            .database()
            .playable_source(source_id)
            .await
            .expect("read")
            .expect("present")
            .work_id;

        record_position(
            &state,
            user_id,
            work_id,
            Millis::new(60_000),
            datetime!(2026-01-01 12:00 UTC),
        )
        .await
        .expect("recorded");

        assert_eq!(
            state
                .database()
                .playback_progress(user_id, work_id)
                .await
                .expect("read")
                .expect("present")
                .state,
            PlaybackState::InProgress,
            "a minute into a two hour film is not the end of it"
        );
    }
}
