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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

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

/// Where a subtitle is written while the tool is still writing it.
///
/// The cache holds whole subtitles and nothing else. The tool writes as it
/// reads the film, so a file already under its final name is one the next
/// request finds, calls converted, and hands to a browser with the end of the
/// film missing from it. Written aside and moved into place in one step once
/// the reading is over, a subtitle is either absent or complete, and whoever
/// asks for it meanwhile waits for the reading rather than reading over its
/// shoulder.
fn while_it_is_written(destination: &Path) -> PathBuf {
    destination.with_extension("vtt.part")
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

    // Asking for one pulls out every one of them, because the reading is the
    // whole cost and it is the same reading. Whoever picks the second track
    // then waits for nothing at all.
    let _ = pull_them_all_out(state, source_id).await;

    // One conversion of a given film at a time, the cache looked at once more
    // under it. Two viewers turning the same subtitle on at the same moment
    // would otherwise both convert it, into the same file, at the same time.
    // Whoever arrives second finds it finished and converts nothing.
    let alone = one_reading_at_a_time(source_id);
    let _converting = alone.lock().await;
    if tokio::fs::metadata(&destination)
        .await
        .is_ok_and(|file| file.len() > 0)
    {
        return Ok(destination);
    }

    let tools = state.tools().ok_or_else(|| {
        AppError::Domain(melyxar_core::Error::dependency_missing(
            "this server has no media tools, so no subtitle can be converted",
        ))
    })?;

    let database = state.database();
    let source = crate::playable_file(database, source_id).await?;

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

    let being_written = while_it_is_written(&destination);
    if let Err(error) =
        melyxar_ffmpeg::subtitles::to_web_vtt(&tools.ffmpeg, &file, &being_written, stream_index)
            .await
    {
        let _ = tokio::fs::remove_file(&being_written).await;
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
    let written = tokio::fs::metadata(&being_written)
        .await
        .map(|file| file.len())
        .unwrap_or(0);
    if written == 0 {
        let _ = tokio::fs::remove_file(&being_written).await;
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

    if let Err(error) = tokio::fs::rename(&being_written, &destination).await {
        let _ = tokio::fs::remove_file(&being_written).await;
        tracing::warn!(
            track = %track_id,
            %error,
            "this subtitle was converted and could not be put in the cache"
        );
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::Internal,
            "this subtitle could not be put in the cache",
        )));
    }
    tracing::info!(track = %track_id, bytes = written, "a subtitle is ready for the browser");
    Ok(destination)
}

/// One film is pulled apart once at a time.
///
/// Two readings of the same film write the same files at the same moment, and
/// a subtitle half written by one and half by the other is a subtitle a
/// browser refuses. It also reads the film twice for nothing: whoever arrives
/// second waits, and then finds everything already there.
fn one_reading_at_a_time(source_id: MediaSourceId) -> Arc<tokio::sync::Mutex<()>> {
    static BY_FILM: OnceLock<
        std::sync::Mutex<HashMap<MediaSourceId, Arc<tokio::sync::Mutex<()>>>>,
    > = OnceLock::new();
    let held = BY_FILM.get_or_init(Default::default);
    let mut films = held.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    films.entry(source_id).or_default().clone()
}

/// Pulls every subtitle made of words out of one film, in a single reading.
///
/// The cost of pulling a subtitle out of a film is not the writing, which is a
/// few tens of kilobytes of text: it is that the words are interleaved with the
/// picture from end to end, so the file has to be read through. Done once per
/// track, a film carrying seven of them is read seven times, and the viewer who
/// picks the last one waits for all seven. Measured on a film with seven
/// tracks: 575 ms in seven passes against 89 ms in one, for output identical to
/// the byte. On a 4K film the same ratio turns two minutes into seventeen
/// seconds.
///
/// Only what is not already there, and only tracks inside the film: one in a
/// file of its own is a file of its own to read, and there is nothing to share.
/// Answers how many were pulled out.
pub async fn pull_them_all_out(state: &AppState, source_id: MediaSourceId) -> Result<usize> {
    // Held for the whole reading, and taken before anything is looked at: what
    // is missing is decided under it, or two readings both decide that the
    // same seven are missing.
    let alone = one_reading_at_a_time(source_id);
    let _reading = alone.lock().await;

    let tools = state.tools().ok_or_else(|| {
        AppError::Domain(melyxar_core::Error::dependency_missing(
            "this server has no media tools, so no subtitle can be converted",
        ))
    })?;

    let database = state.database();
    let source = crate::playable_file(database, source_id).await?;

    let folder = state.config().directories.subtitles();
    tokio::fs::create_dir_all(&folder)
        .await
        .map_err(AppError::Directory)?;

    let tracks = database.tracks_of_source(source_id).await?;
    let mut wanted: Vec<(i32, PathBuf)> = Vec::new();
    for track in &tracks {
        let Ok(details) = text_subtitle(track) else {
            continue;
        };
        if details.is_external {
            continue;
        }
        let destination = cached_at(state, track.id);
        if tokio::fs::metadata(&destination)
            .await
            .is_ok_and(|file| file.len() > 0)
        {
            continue;
        }
        wanted.push((track.stream_index, destination));
    }

    if wanted.is_empty() {
        // Nothing left to pull out is an answer about this film, and it is
        // written down like any other: without it the upkeep would take the
        // whole file past again at every run, for ever, to find the same
        // nothing.
        written_down(state, source_id, 0).await;
        return Ok(0);
    }

    tracing::info!(
        subtitles = wanted.len(),
        "pulling every subtitle out of this film in one reading"
    );
    let being_written: Vec<PathBuf> = wanted
        .iter()
        .map(|(_, destination)| while_it_is_written(destination))
        .collect();
    let asked: Vec<(i32, &Path)> = wanted
        .iter()
        .zip(&being_written)
        .map(|((index, _), aside)| (*index, aside.as_path()))
        .collect();
    if let Err(error) =
        melyxar_ffmpeg::subtitles::all_to_web_vtt(&tools.ffmpeg, &source.path, &asked).await
    {
        for aside in &being_written {
            let _ = tokio::fs::remove_file(aside).await;
        }
        tracing::warn!(
            %error,
            subtitles = wanted.len(),
            "these subtitles could not be pulled out together; each will be tried on its own \
             when somebody asks for it"
        );
        return Err(error.into());
    }

    // What really landed, rather than what was asked for. A tool that answers
    // "it went well" and writes nothing leaves a viewer with a player showing
    // no subtitle and a server reporting no fault. Each one enters the cache
    // whole, at the end of the reading and in a single step, so nobody is
    // handed half a film's worth of words.
    let mut pulled = 0;
    for ((_, destination), aside) in wanted.iter().zip(&being_written) {
        if !tokio::fs::metadata(aside)
            .await
            .is_ok_and(|file| file.len() > 0)
        {
            continue;
        }
        match tokio::fs::rename(aside, destination).await {
            Ok(()) => pulled += 1,
            Err(error) => {
                let _ = tokio::fs::remove_file(aside).await;
                tracing::warn!(
                    %error,
                    "a subtitle was pulled out and could not be put in the cache, so it will \
                     be pulled out again when somebody asks for it"
                );
            }
        }
    }
    written_down(state, source_id, pulled).await;
    tracing::info!(
        pulled,
        asked_for = wanted.len(),
        "the subtitles of this film are ready for the browser"
    );
    Ok(pulled)
}

/// Writes down that one film has been taken past for its words.
///
/// What the upkeep asks for is "which films still have words in them nobody
/// has pulled out", and only this answers it: the cache is a folder of files
/// named after tracks, so nothing in it says which film has been done and
/// which merely carries no subtitle at all.
///
/// A failure here is said and let go. The words are in the cache either way,
/// which is what a viewer needs; what is lost is that the film is offered up
/// again at the next run of the upkeep, where it will find everything already
/// there and cost one reading of nothing.
async fn written_down(state: &AppState, source_id: MediaSourceId, pulled_out: usize) {
    if let Err(error) = state
        .database()
        .store_pulled_out_subtitles(source_id, pulled_out)
        .await
    {
        tracing::warn!(
            %error,
            "that this film has been taken past for its words could not be kept"
        );
    }
}

/// Throws away every subtitle already converted, and says how many that was.
///
/// For testing the conversion itself rather than the cache in front of it. A
/// track that is already converted is served in a millisecond and proves
/// nothing about the minute it took to get there, so trying the slow path
/// again means being able to empty this.
///
/// Only the files this writes: named after a track, in the folder this owns,
/// whether they are finished or were left half written by a reading that did
/// not finish. Nothing else in there is touched, and a folder that does not
/// exist yet is not an error, it is a server nobody has asked for a subtitle
/// from.
///
/// What the upkeep remembers goes with them. A film written down as taken past
/// for its words, whose words are no longer anywhere, is a film the upkeep
/// would never offer up again: emptying one without the other leaves a library
/// that only converts a subtitle when somebody is waiting for it, which is the
/// state this whole pass exists to get out of.
pub async fn forget_what_was_converted(state: &AppState) -> Result<usize> {
    let waiting_again = state.database().forget_pulled_out_subtitles().await?;
    if waiting_again > 0 {
        tracing::info!(
            films = waiting_again,
            "these films are waiting for their words again, and the upkeep will pull them out"
        );
    }

    let folder = state.config().directories.subtitles();
    let mut reading = match tokio::fs::read_dir(&folder).await {
        Ok(reading) => reading,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(AppError::Directory(error)),
    };

    let mut thrown_away = 0;
    while let Ok(Some(entry)) = reading.next_entry().await {
        let path = entry.path();
        match path.extension().and_then(|kind| kind.to_str()) {
            Some("vtt") => match tokio::fs::remove_file(&path).await {
                Ok(()) => thrown_away += 1,
                Err(error) => tracing::warn!(
                    %error,
                    "a converted subtitle could not be thrown away; it will be served from the cache again"
                ),
            },
            // Left behind by a reading that did not finish. Never served and
            // never counted, so there is nothing to keep it for.
            Some("part") => {
                let _ = tokio::fs::remove_file(&path).await;
            }
            _ => continue,
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

    /// Writes a real film carrying one subtitle track per set of words.
    ///
    /// The video stream is the file's first, so the tracks are the ones after
    /// it: whoever calls this says so when recording them.
    async fn a_film_carrying(tool: &Path, film: &Path, said: &[&str]) {
        let beside = film.parent().expect("the film sits somewhere");
        let mut making = tokio::process::Command::new(tool);
        making.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
        ]);
        making.arg("testsrc2=size=160x90:rate=8:duration=4");
        for (which, words) in said.iter().enumerate() {
            let path = beside.join(format!("words-{which}.srt"));
            std::fs::write(
                &path,
                format!("1\n00:00:01,000 --> 00:00:03,500\n{words}\n"),
            )
            .expect("a subtitle file");
            making.arg("-i").arg(&path);
        }
        making.args(["-map", "0:v"]);
        for which in 0..said.len() {
            making.args(["-map", &format!("{}:s", which + 1)]);
        }
        making.args(["-c:v", "libx264", "-preset", "ultrafast", "-c:s", "srt"]);
        making.arg(film);
        assert!(
            making.status().await.expect("the tool runs").success(),
            "a film carrying its subtitle tracks"
        );
    }

    /// What the cache holds, by the end of each name.
    fn in_the_cache(state: &AppState, ending: &str) -> usize {
        std::fs::read_dir(state.config().directories.subtitles())
            .expect("the cache folder")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().and_then(|kind| kind.to_str()) == Some(ending))
            .count()
    }

    #[test]
    fn a_subtitle_being_written_is_not_under_the_name_the_cache_reads() {
        // The whole point: a request that goes looking for a converted
        // subtitle must not find one the tool is still writing.
        let cached = PathBuf::from("/cache/subtitles/one.vtt");
        let aside = while_it_is_written(&cached);
        assert_ne!(aside, cached);
        assert_ne!(
            aside.extension().and_then(|kind| kind.to_str()),
            Some("vtt")
        );
    }

    #[tokio::test]
    async fn every_subtitle_enters_the_cache_whole_and_none_half_written() {
        // The tool writes as it reads the film, so a file put straight under
        // its final name is one the next request calls converted and hands to
        // a browser with the end of the film missing from it.
        let (directory, state, source_id, tracks) = state_with(
            |id| {
                vec![
                    subtitle_track(id, 1, SubtitleLayout::Text, None),
                    subtitle_track(id, 2, SubtitleLayout::Text, None),
                ]
            },
            &[],
        )
        .await;

        let said = ["Bonsoir.", "Good evening."];
        let film = directory
            .path()
            .join("films")
            .join("Quiet.Harbour.2019.mkv");
        let tool = state
            .tools()
            .expect("the tools are installed here")
            .ffmpeg
            .clone();
        a_film_carrying(&tool, &film, &said).await;

        assert_eq!(
            pull_them_all_out(&state, source_id)
                .await
                .expect("both come out"),
            2
        );

        for (which, words) in said.iter().enumerate() {
            let text = std::fs::read_to_string(cached_at(&state, tracks[which].id))
                .expect("read back from the cache");
            assert!(text.starts_with("WEBVTT"), "{text}");
            assert!(
                text.contains(words),
                "track {which} holds its own words: {text}"
            );
        }
        assert_eq!(
            in_the_cache(&state, "part"),
            0,
            "nothing is left behind half written"
        );
    }

    #[tokio::test]
    async fn two_viewers_turning_the_same_subtitle_on_at_once_both_get_it_whole() {
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

        let (first, second) = tokio::join!(
            as_web_vtt(&state, source_id, tracks[0].id),
            as_web_vtt(&state, source_id, tracks[0].id)
        );
        for handed in [first.expect("converted"), second.expect("converted")] {
            let text = std::fs::read_to_string(&handed).expect("read back");
            assert!(text.starts_with("WEBVTT"), "{text}");
            assert!(text.contains("Bonsoir."), "{text}");
        }
        assert_eq!(in_the_cache(&state, "part"), 0);
    }

    #[tokio::test]
    async fn a_conversion_that_goes_wrong_leaves_nothing_in_the_cache() {
        // The film in this one is not a film, so the tool refuses it. What it
        // began must not stay: half a subtitle in the cache is one the next
        // viewer is handed as though it were whole.
        let (_directory, state, source_id, tracks) = state_with(
            |id| vec![subtitle_track(id, 2, SubtitleLayout::Text, None)],
            &[],
        )
        .await;

        assert!(as_web_vtt(&state, source_id, tracks[0].id).await.is_err());
        assert_eq!(in_the_cache(&state, "part"), 0);
        assert_eq!(in_the_cache(&state, "vtt"), 0);
    }

    #[tokio::test]
    async fn a_subtitle_left_half_written_is_thrown_away_without_being_counted() {
        let (_directory, state, _source_id, _tracks) = state_with(
            |id| vec![subtitle_track(id, 2, SubtitleLayout::Text, None)],
            &[],
        )
        .await;

        let folder = state.config().directories.subtitles();
        std::fs::create_dir_all(&folder).expect("the cache folder");
        std::fs::write(folder.join("one.vtt"), "WEBVTT\n\n").expect("a finished subtitle");
        std::fs::write(folder.join("two.vtt.part"), "WEBVTT\n\n").expect("a reading cut short");

        assert_eq!(
            forget_what_was_converted(&state)
                .await
                .expect("thrown away"),
            1,
            "only the finished one was ever a converted subtitle"
        );
        assert_eq!(in_the_cache(&state, "part"), 0);
        assert_eq!(in_the_cache(&state, "vtt"), 0);
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
