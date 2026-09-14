//! Handing a subtitle track to a browser.
//!
//! A browser draws WebVTT and nothing else, so everything a film carries has
//! to be converted first: SubRip inside the file, an .srt sitting beside it,
//! ASS with its own styling. The conversion is cheap and the answer is small,
//! so it is written once to the cache and read from there afterwards.
//!
//! Subtitles made of pictures never reach here. There is no text in them to
//! convert, and the playback decision already sends those the other way: they
//! are drawn into the picture, which is what forces a full rebuild.

use std::path::PathBuf;

use melyxar_core::id::{MediaSourceId, TrackId};
use melyxar_core::media::{SubtitleDetails, SubtitleLayout, Track, TrackKind};

use crate::{AppError, AppState, Result};

/// Where one converted subtitle track is kept.
///
/// Named after the track rather than the film: one film can carry a dozen, and
/// a viewer switching between two of them should find the second already
/// there.
fn cached_at(state: &AppState, track_id: TrackId) -> PathBuf {
    state
        .config()
        .directories
        .subtitles()
        .join(format!("{track_id}.vtt"))
}

/// Converts one subtitle track to WebVTT, unless it was converted already.
///
/// Answers where the file is. The caller serves it; nothing here knows what a
/// range request is.
pub async fn as_web_vtt(
    state: &AppState,
    source_id: MediaSourceId,
    track_id: TrackId,
) -> Result<PathBuf> {
    let destination = cached_at(state, track_id);
    if tokio::fs::metadata(&destination)
        .await
        .is_ok_and(|file| file.len() > 0)
    {
        tracing::debug!(track = %track_id, "a subtitle was already converted and is served from the cache");
        return Ok(destination);
    }

    let tools = state.tools().ok_or_else(|| {
        AppError::Domain(melyxar_core::Error::dependency_missing(
            "this server has no media tools, so no subtitle can be converted",
        ))
    })?;

    let database = state.database();
    let source = database
        .playable_source(source_id)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("media source")))?;
    if source.missing {
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::RootUnavailable,
            "the file is not on the disk at the moment",
        )));
    }

    let tracks = database.tracks_of_source(source_id).await?;
    let Some(track) = tracks.iter().find(|track| track.id == track_id) else {
        // Said out loud with what the file does carry: a track asked for and
        // not found means the page and the database disagree, and the only way
        // to see that is to print both sides.
        tracing::warn!(
            track = %track_id,
            subtitles_this_file_carries = tracks
                .iter()
                .filter(|track| matches!(track.kind, TrackKind::Subtitle(_)))
                .count(),
            "a subtitle track was asked for that this file does not carry"
        );
        return Err(AppError::Domain(melyxar_core::Error::not_found(
            "subtitle track",
        )));
    };
    let details = text_subtitle(track)?;

    // A track in a file of its own is taken whole; one inside the film is
    // named by its place in it. The relative path is the one the scan
    // recorded, and it is joined to the root here rather than trusted from
    // anywhere else.
    let (file, stream_index) = match &details.external_relative_path {
        Some(relative) if details.is_external => (source.root.join(relative), None),
        _ => (source.path.clone(), Some(track.stream_index)),
    };

    if let Some(folder) = destination.parent() {
        tokio::fs::create_dir_all(folder)
            .await
            .map_err(AppError::Directory)?;
    }

    tracing::info!(
        track = %track_id,
        codec = details.codec,
        language = track.language.as_deref().unwrap_or("none"),
        in_its_own_file = details.is_external,
        stream = stream_index.unwrap_or(-1),
        "converting a subtitle for the browser"
    );

    if let Err(error) =
        melyxar_ffmpeg::subtitles::to_web_vtt(&tools.ffmpeg, &file, &destination, stream_index)
            .await
    {
        tracing::warn!(
            track = %track_id,
            codec = details.codec,
            %error,
            "this subtitle could not be converted, so nothing will be shown"
        );
        return Err(error.into());
    }

    // A tool that answers "it went well" and writes nothing leaves a viewer
    // with a player that shows no subtitle and a server that reported no
    // fault. Measured rather than assumed, because that silence is exactly
    // what nobody can work out from a screen.
    let written = tokio::fs::metadata(&destination)
        .await
        .map(|file| file.len())
        .unwrap_or(0);
    if written == 0 {
        tracing::warn!(
            track = %track_id,
            codec = details.codec,
            stream = stream_index.unwrap_or(-1),
            "the tool converted this subtitle without complaining and wrote nothing, \
             so the player will show no subtitle at all"
        );
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::Internal,
            "this subtitle came back empty",
        )));
    }
    tracing::info!(track = %track_id, bytes = written, "a subtitle is ready for the browser");
    Ok(destination)
}

/// Throws away every subtitle already converted, and says how many that was.
///
/// For testing the conversion itself rather than the cache in front of it. A
/// track that is already converted is served in a millisecond and proves
/// nothing about the minute it took to get there, so trying the slow path
/// again means being able to empty this.
///
/// Only the files this writes: named after a track and ending in `.vtt`, in
/// the folder this owns. Nothing else in there is touched, and a folder that
/// does not exist yet is not an error, it is a server nobody has asked for a
/// subtitle from.
pub async fn forget_what_was_converted(state: &AppState) -> Result<usize> {
    let folder = state.config().directories.subtitles();
    let mut reading = match tokio::fs::read_dir(&folder).await {
        Ok(reading) => reading,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(AppError::Directory(error)),
    };

    let mut thrown_away = 0;
    while let Ok(Some(entry)) = reading.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|kind| kind.to_str()) != Some("vtt") {
            continue;
        }
        match tokio::fs::remove_file(&path).await {
            Ok(()) => thrown_away += 1,
            Err(error) => tracing::warn!(
                %error,
                "a converted subtitle could not be thrown away; it will be served from the cache again"
            ),
        }
    }
    tracing::info!(
        subtitles = thrown_away,
        "converted subtitles were thrown away, so the next one asked for is converted afresh"
    );
    Ok(thrown_away)
}

/// The track, when it is a subtitle made of text.
fn text_subtitle(track: &Track) -> Result<&SubtitleDetails> {
    let TrackKind::Subtitle(details) = &track.kind else {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "that track is not a subtitle",
        )));
    };
    if details.layout != SubtitleLayout::Text {
        // There is no text in a picture to convert. The playback decision
        // already draws these into the picture instead, and saying so beats
        // handing back an empty file.
        tracing::warn!(
            track = %track.id,
            codec = details.codec,
            "this subtitle is made of pictures, so it cannot be handed to the browser \
             on its own: it has to be drawn into the film, which the playback decision \
             asks for"
        );
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "this subtitle is made of pictures and can only be drawn into the film",
        )));
    }
    Ok(details)
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::media::{AudioDetails, Loudness};
    use melyxar_core::user::Permissions;
    use melyxar_database::Database;

    fn subtitle_track(
        source_id: MediaSourceId,
        stream_index: i32,
        layout: SubtitleLayout,
        external: Option<&str>,
    ) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index,
            language: Some("fre".into()),
            title: None,
            is_default: false,
            is_forced: false,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: "subrip".into(),
                layout,
                is_hearing_impaired: false,
                is_external: external.is_some(),
                external_relative_path: external.map(PathBuf::from),
            }),
        }
    }

    fn audio_track(source_id: MediaSourceId) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 1,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: "aac".into(),
                profile: None,
                channels: 2,
                channel_layout: Some("stereo".into()),
                sample_rate: Some(48_000),
                bit_depth: None,
                bitrate: None,
                loudness: Loudness::default(),
            }),
        }
    }

    /// One film in one library, with whatever tracks a test needs.
    async fn state_with(
        tracks: impl FnOnce(MediaSourceId) -> Vec<Track>,
        alongside: &[(&str, &str)],
    ) -> (tempfile::TempDir, AppState, MediaSourceId, Vec<Track>) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        std::fs::create_dir_all(&media).expect("media folder");
        std::fs::write(media.join("Quiet.Harbour.2019.mkv"), b"not a real film")
            .expect("the film file");
        for (name, contents) in alongside {
            std::fs::write(media.join(name), contents).expect("a file beside the film");
        }

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
                std::path::Path::new("Quiet.Harbour.2019.mkv"),
                12_000,
                melyxar_core::time::now(),
            )
            .await
            .expect("source recorded");
        let made = tracks(source_id);
        database
            .store_analysis(
                source_id,
                &melyxar_database::catalogue::SourceAnalysis {
                    container: Some("matroska,webm".into()),
                    duration: Some(melyxar_core::time::Millis::new(7_200_000)),
                    overall_bitrate: None,
                },
                &made,
                &[],
            )
            .await
            .expect("analysis stored");
        database
            .create_user("victor", None, &Permissions::administrator())
            .await
            .expect("account created");

        let (tools, capabilities) = crate::startup::detect_media_tools(&config).await;
        let state = AppState::new(config, database, tools, capabilities);
        (directory, state, source_id, made)
    }

    #[tokio::test]
    async fn a_subtitle_file_beside_the_film_comes_out_as_something_a_browser_draws() {
        let (_directory, state, source_id, tracks) = state_with(
            |id| {
                vec![subtitle_track(
                    id,
                    0,
                    SubtitleLayout::Text,
                    Some("Quiet.Harbour.2019.fr.srt"),
                )]
            },
            &[(
                "Quiet.Harbour.2019.fr.srt",
                "1\n00:00:01,000 --> 00:00:03,500\nBonsoir.\n",
            )],
        )
        .await;

        let written = as_web_vtt(&state, source_id, tracks[0].id)
            .await
            .expect("converted");
        let text = std::fs::read_to_string(&written).expect("read back");
        assert!(text.starts_with("WEBVTT"));
        assert!(text.contains("Bonsoir."));
    }

    #[tokio::test]
    async fn a_track_converted_once_is_not_converted_again() {
        // A film has one set of subtitles and many viewers. Converting them
        // for each of them is work nobody asked for.
        let (_directory, state, source_id, tracks) = state_with(
            |id| {
                vec![subtitle_track(
                    id,
                    0,
                    SubtitleLayout::Text,
                    Some("Quiet.Harbour.2019.fr.srt"),
                )]
            },
            &[(
                "Quiet.Harbour.2019.fr.srt",
                "1\n00:00:01,000 --> 00:00:03,500\nBonsoir.\n",
            )],
        )
        .await;

        let first = as_web_vtt(&state, source_id, tracks[0].id)
            .await
            .expect("converted");
        // Written over with something the tool would never produce: if it ran
        // again, this would be gone.
        std::fs::write(&first, "WEBVTT\n\n00:01.000 --> 00:03.000\nkept\n").expect("written over");

        let again = as_web_vtt(&state, source_id, tracks[0].id)
            .await
            .expect("found again");
        assert_eq!(first, again);
        assert!(
            std::fs::read_to_string(&again)
                .expect("read back")
                .contains("kept"),
            "the answer already on the disk is the one handed over"
        );
    }

    #[tokio::test]
    async fn a_subtitle_made_of_pictures_says_so_rather_than_handing_back_nothing() {
        let (_directory, state, source_id, tracks) = state_with(
            |id| vec![subtitle_track(id, 2, SubtitleLayout::Bitmap, None)],
            &[],
        )
        .await;

        let failure = as_web_vtt(&state, source_id, tracks[0].id)
            .await
            .expect_err("there is no text in a picture to convert");
        assert!(matches!(
            failure,
            AppError::Domain(error) if error.code == melyxar_core::error::ErrorCode::InvalidInput
        ));
    }

    #[tokio::test]
    async fn a_track_that_is_not_a_subtitle_is_refused() {
        let (_directory, state, source_id, tracks) =
            state_with(|id| vec![audio_track(id)], &[]).await;

        assert!(as_web_vtt(&state, source_id, tracks[0].id).await.is_err());
    }

    #[tokio::test]
    async fn a_track_this_film_does_not_carry_is_refused_rather_than_looked_for() {
        let (_directory, state, source_id, _tracks) = state_with(
            |id| vec![subtitle_track(id, 2, SubtitleLayout::Text, None)],
            &[],
        )
        .await;

        let failure = as_web_vtt(&state, source_id, TrackId::new())
            .await
            .expect_err("nothing by that name");
        assert!(matches!(
            failure,
            AppError::Domain(error) if error.code == melyxar_core::error::ErrorCode::NotFound
        ));
    }

    #[tokio::test]
    async fn each_track_of_one_film_is_kept_apart_from_the_others() {
        // A film carries a dozen, and a viewer switching between two of them
        // should find the second one already there rather than the first.
        let (_directory, state, _source_id, tracks) = state_with(
            |id| {
                vec![
                    subtitle_track(id, 2, SubtitleLayout::Text, None),
                    subtitle_track(id, 3, SubtitleLayout::Text, None),
                ]
            },
            &[],
        )
        .await;

        assert_ne!(
            cached_at(&state, tracks[0].id),
            cached_at(&state, tracks[1].id)
        );
    }
}
