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

use melyxar_core::id::{MediaSourceId, TrackId, UserId, WorkId};
use melyxar_core::media::Track;
use melyxar_core::time::{Millis, Timestamp};
use melyxar_core::work::{state_for_position, PlaybackState, DEFAULT_WATCHED_THRESHOLD};
use melyxar_playback::decision::{decide, PlaybackDecision, PlaybackRequest};
use melyxar_playback::profile::ClientProfile;

use crate::{AppError, AppState, Result};

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
    let audio = request
        .audio_track_id
        .and_then(|id| tracks.iter().find(|track| track.id == id));
    let subtitle = request
        .subtitle_track_id
        .and_then(|id| tracks.iter().find(|track| track.id == id));

    let preferences = database.user(user_id).await?.map(|user| user.preferences);
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

    let resume_from = database
        .playback_progress(user_id, source.work_id)
        .await?
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
