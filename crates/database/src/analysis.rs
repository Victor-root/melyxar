//! What an analysis found inside a file: its streams, its chapters and the
//! subtitles that live beside it, or why it could not be described.

use std::path::PathBuf;

use melyxar_core::id::{ChapterId, LibraryId, LibraryRootId, MediaSourceId, TrackId};
use melyxar_core::media::{
    AudioDetails, Chapter, ColorInfo, HdrFormat, Loudness, Margins, SubtitleDetails,
    SubtitleLayout, Track, TrackKind, VideoDetails,
};
use melyxar_core::time::{now, Millis};
use sqlx::Row;

use crate::catalogue::SourceAnalysis;
use crate::convert::{bool_to_int, int_to_bool, parse_id, timestamp_to_text};
use crate::openings::insert_segment;
use crate::{Database, DatabaseError, Result};

/// How much of a refusal is worth keeping.
///
/// Long enough to carry what the tool named and where, short enough that a
/// report stays a report.
const LONGEST_FAILURE_REASON: usize = 400;

impl Database {
    /// Writes down why the analyser could not describe a file.
    ///
    /// Kept rather than logged: a reason in a log line is gone by the time
    /// anybody asks, and this is the one thing that says what to do about a
    /// file that has a card in the library and fails when it is played.
    pub async fn record_analysis_failure(
        &self,
        source_id: MediaSourceId,
        reason: &str,
    ) -> Result<()> {
        write_analysis_failure(self.writer(), source_id, reason).await
    }

    /// Forgets what the analyser found, so the next scan reads every file
    /// again.
    ///
    /// An analysis is kept once it is done, which is what makes a second scan
    /// cost almost nothing. But what the analyser is asked to read grows: a
    /// collection analysed by an older build carries the gaps that build left,
    /// and nothing would ever look at those files again. This is how somebody
    /// asks for them to be read once more, and it costs one scan.
    ///
    /// The files themselves are untouched, and so is everything attached to
    /// them: only what was read out of them goes.
    ///
    /// All of it goes, not merely the mark saying it was read. A file left
    /// with the container an older reading found and no streams to go with it
    /// describes itself as something it was never shown to be, and whatever
    /// reads that afterwards believes it.
    pub async fn forget_analysis(&self, library_id: LibraryId) -> Result<u64> {
        let done = sqlx::query(
            "UPDATE media_sources
             SET analysed_at = NULL, analysis_failure = NULL, container = NULL,
                 duration_ms = NULL, overall_bitrate = NULL
             WHERE root_id IN (SELECT id FROM library_roots WHERE library_id = ?)",
        )
        .bind(library_id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(done.rows_affected())
    }

    /// Stores what the analysis found: the file as a whole, its streams and
    /// its chapters, in one transaction so a reader never sees half of it.
    pub async fn store_analysis(
        &self,
        source_id: MediaSourceId,
        analysis: &SourceAnalysis,
        tracks: &[Track],
        chapters: &[Chapter],
    ) -> Result<()> {
        let mut transaction = self.begin().await?;
        write_analysis(&mut transaction, source_id, analysis, tracks, chapters).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Files of one root that already carry a subtitle of their own.
    ///
    /// A scan asks for this so that it only rewrites what actually changed:
    /// without it, either a removed subtitle would linger for ever or every
    /// file in the library would be written to on every scan.
    pub async fn sources_with_external_subtitles(
        &self,
        root_id: LibraryRootId,
    ) -> Result<Vec<MediaSourceId>> {
        let rows = sqlx::query(
            "SELECT DISTINCT tracks.source_id FROM tracks
             JOIN media_sources ON media_sources.id = tracks.source_id
             WHERE media_sources.root_id = ? AND tracks.is_external = 1
               AND tracks.downloaded_file IS NULL",
        )
        .bind(root_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| parse_id(&row.try_get::<String, _>("source_id")?))
            .collect()
    }

    /// Replaces the subtitles that live in their own files next to a source.
    ///
    /// Kept apart from the analysis, which owns the streams inside the file:
    /// each writes what it knows about and leaves the rest alone, so a scan
    /// and an analysis can happen in either order.
    pub async fn store_external_subtitles(
        &self,
        source_id: MediaSourceId,
        tracks: &[Track],
    ) -> Result<()> {
        let mut transaction = self.begin().await?;
        sqlx::query(
            "DELETE FROM tracks
              WHERE source_id = ? AND is_external = 1 AND downloaded_file IS NULL",
        )
            .bind(source_id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        for track in tracks {
            insert_track(&mut transaction, source_id, track).await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Adds a subtitle downloaded for a source, as one more of its tracks.
    pub async fn add_downloaded_subtitle(&self, source_id: MediaSourceId, track: &Track) -> Result<()> {
        let mut transaction = self.begin().await?;
        insert_track(&mut transaction, source_id, track).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// The files of every subtitle downloaded that a track still holds.
    pub async fn downloaded_subtitle_files(&self) -> Result<Vec<String>> {
        Ok(sqlx::query_scalar(
            "SELECT downloaded_file FROM tracks WHERE downloaded_file IS NOT NULL",
        )
        .fetch_all(self.reader())
        .await?)
    }

    /// Takes away a subtitle downloaded for this source, and answers the name
    /// of its file so it can be deleted. Nothing for a track that is not one.
    pub async fn remove_downloaded_subtitle(
        &self,
        source_id: MediaSourceId,
        track_id: TrackId,
    ) -> Result<Option<String>> {
        let mut transaction = self.begin().await?;
        let file: Option<String> = sqlx::query_scalar(
            "SELECT downloaded_file FROM tracks
              WHERE id = ? AND source_id = ? AND downloaded_file IS NOT NULL",
        )
        .bind(track_id.to_db_string())
        .bind(source_id.to_db_string())
        .fetch_optional(&mut *transaction)
        .await?;
        if file.is_some() {
            sqlx::query("DELETE FROM tracks WHERE id = ?")
                .bind(track_id.to_db_string())
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(file)
    }

    /// Streams of one source, video first, then audio, then subtitles.
    pub async fn tracks_of_source(&self, source_id: MediaSourceId) -> Result<Vec<Track>> {
        let rows = sqlx::query(
            "SELECT * FROM tracks WHERE source_id = ?
             ORDER BY CASE kind WHEN 'video' THEN 0 WHEN 'audio' THEN 1 ELSE 2 END, stream_index",
        )
        .bind(source_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter().map(track_from_row).collect()
    }

    /// Chapters of one source, in order.
    pub async fn chapters_of_source(&self, source_id: MediaSourceId) -> Result<Vec<Chapter>> {
        let rows = sqlx::query(
            "SELECT ordinal, start_ms, title, thumbnail_path FROM chapters
             WHERE source_id = ? ORDER BY ordinal",
        )
        .bind(source_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        let mut chapters = Vec::with_capacity(rows.len());
        for row in rows {
            chapters.push(Chapter {
                ordinal: row.try_get("ordinal")?,
                start: Millis::new(row.try_get("start_ms")?),
                title: row.try_get("title")?,
                thumbnail_path: row
                    .try_get::<Option<String>, _>("thumbnail_path")?
                    .map(PathBuf::from),
            });
        }
        Ok(chapters)
    }
}

/// Writes down why a file could not be described, cut to a length a report
/// can show. What the tool says first is what says why; the rest is the same
/// complaint again.
pub(crate) async fn write_analysis_failure<'e, E>(
    executor: E,
    source_id: MediaSourceId,
    reason: &str,
) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let reason: String = reason.chars().take(LONGEST_FAILURE_REASON).collect();
    sqlx::query("UPDATE media_sources SET analysis_failure = ? WHERE id = ?")
        .bind(reason)
        .bind(source_id.to_db_string())
        .execute(executor)
        .await?;
    Ok(())
}

/// Writes what one analysis found, inside a transaction the caller holds:
/// the file as a whole, its streams and its chapters, replacing what an
/// earlier reading wrote.
pub(crate) async fn write_analysis(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    source_id: MediaSourceId,
    analysis: &SourceAnalysis,
    tracks: &[Track],
    chapters: &[Chapter],
) -> Result<()> {
    sqlx::query(
        "UPDATE media_sources
         SET container = ?, duration_ms = ?, overall_bitrate = ?, analysed_at = ?,
             analysis_failure = NULL
         WHERE id = ?",
    )
    .bind(analysis.container.as_deref())
    .bind(analysis.duration.map(Millis::get))
    .bind(analysis.overall_bitrate)
    .bind(timestamp_to_text(now()))
    .bind(source_id.to_db_string())
    .execute(&mut **transaction)
    .await?;

    // An analysis owns the streams inside the file and nothing else: the
    // subtitles that live in their own files are not its to replace.
    sqlx::query("DELETE FROM tracks WHERE source_id = ? AND is_external = 0")
        .bind(source_id.to_db_string())
        .execute(&mut **transaction)
        .await?;
    sqlx::query("DELETE FROM chapters WHERE source_id = ?")
        .bind(source_id.to_db_string())
        .execute(&mut **transaction)
        .await?;
    // The stretches read off those chapters go with them. Anything a
    // person said themselves stays: a correction made by hand is not the
    // analysis's to undo, and reading the file again is not somebody
    // changing their mind.
    sqlx::query("DELETE FROM media_segments WHERE source_id = ? AND origin <> 'manual'")
        .bind(source_id.to_db_string())
        .execute(&mut **transaction)
        .await?;
    // And with the stretches found by listening goes the note saying this
    // file was listened to, for the same reason a film whose thumbnails
    // were thrown away is written down as having none: a file marked as
    // done with nothing to show for it is a file that is never read again,
    // and its season would keep its silence for ever.
    sqlx::query("DELETE FROM media_source_openings WHERE source_id = ?")
        .bind(source_id.to_db_string())
        .execute(&mut **transaction)
        .await?;

    for track in tracks {
        insert_track(transaction, source_id, track).await?;
    }
    for chapter in chapters {
        sqlx::query(
            "INSERT INTO chapters (id, source_id, ordinal, start_ms, title, thumbnail_path)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(ChapterId::new().to_db_string())
        .bind(source_id.to_db_string())
        .bind(chapter.ordinal)
        .bind(chapter.start.get())
        .bind(chapter.title.as_deref())
        .bind(
            chapter
                .thumbnail_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
        )
        .execute(&mut **transaction)
        .await?;
    }

    // Read from the chapters that were just written, in the same step:
    // the two are one reading of one file and a reader must never see the
    // chapters of today beside the stretches of yesterday.
    for segment in melyxar_core::segments::segments_from_chapters(chapters, analysis.duration) {
        insert_segment(&mut **transaction, source_id, &segment).await?;
    }

    Ok(())
}

async fn insert_track(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    source_id: MediaSourceId,
    track: &Track,
) -> Result<()> {
    let (codec, profile, level, bitrate) = match &track.kind {
        TrackKind::Video(details) => (
            details.codec.as_str(),
            details.profile.as_deref(),
            details.level,
            details.bitrate,
        ),
        TrackKind::Audio(details) => (
            details.codec.as_str(),
            details.profile.as_deref(),
            None,
            details.bitrate,
        ),
        TrackKind::Subtitle(details) => (details.codec.as_str(), None, None, None),
    };

    let video = match &track.kind {
        TrackKind::Video(details) => Some(details),
        _ => None,
    };
    let audio = match &track.kind {
        TrackKind::Audio(details) => Some(details),
        _ => None,
    };
    let subtitle = match &track.kind {
        TrackKind::Subtitle(details) => Some(details),
        _ => None,
    };

    let (hdr_format, dolby_vision_profile) = match video.and_then(|details| details.hdr) {
        Some(HdrFormat::Hdr10) => (Some("hdr10"), None),
        Some(HdrFormat::Hlg) => (Some("hlg"), None),
        Some(HdrFormat::DolbyVision { profile }) => (Some("dolby_vision"), profile),
        None => (None, None),
    };

    sqlx::query(
        "INSERT INTO tracks (
            id, source_id, stream_index, kind, language, title, is_default, is_forced,
            codec, profile, level, bitrate,
            width, height, aspect_ratio, is_interlaced, frame_rate, pixel_format,
            reference_frames, color_primaries, color_space, color_transfer, bit_depth,
            hdr_format, dolby_vision_profile,
            channels, channel_layout, sample_rate,
            loudness_integrated_lufs, loudness_true_peak_dbfs, loudness_range_lu,
            subtitle_layout, is_hearing_impaired, is_external, external_relative_path,
            downloaded_file, margin_top, margin_bottom, margin_left, margin_right
         ) VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?,
            ?, ?,
            ?, ?, ?,
            ?, ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?, ?, ?
         )",
    )
    .bind(track.id.to_db_string())
    .bind(source_id.to_db_string())
    .bind(track.stream_index)
    .bind(track.kind.as_str())
    .bind(track.language.as_deref())
    .bind(track.title.as_deref())
    .bind(bool_to_int(track.is_default))
    .bind(bool_to_int(track.is_forced))
    .bind(codec)
    .bind(profile)
    .bind(level)
    .bind(bitrate)
    .bind(video.map(|details| details.width))
    .bind(video.map(|details| details.height))
    .bind(video.and_then(|details| details.aspect_ratio.as_deref()))
    .bind(video.map(|details| bool_to_int(details.is_interlaced)))
    .bind(video.and_then(|details| details.frame_rate))
    .bind(video.and_then(|details| details.pixel_format.as_deref()))
    .bind(video.and_then(|details| details.reference_frames))
    .bind(video.and_then(|details| details.color.primaries.as_deref()))
    .bind(video.and_then(|details| details.color.space.as_deref()))
    .bind(video.and_then(|details| details.color.transfer.as_deref()))
    // Sample depth is carried by video colour and by audio alike, and one
    // column holds both because no track is ever of two kinds at once.
    .bind(
        video
            .and_then(|details| details.color.bit_depth)
            .or_else(|| audio.and_then(|details| details.bit_depth)),
    )
    .bind(hdr_format)
    .bind(dolby_vision_profile)
    .bind(audio.map(|details| details.channels))
    .bind(audio.and_then(|details| details.channel_layout.as_deref()))
    .bind(audio.and_then(|details| details.sample_rate))
    .bind(audio.and_then(|details| details.loudness.integrated_lufs))
    .bind(audio.and_then(|details| details.loudness.true_peak_dbfs))
    .bind(audio.and_then(|details| details.loudness.range_lu))
    .bind(subtitle.map(|details| match details.layout {
        SubtitleLayout::Text => "text",
        SubtitleLayout::Bitmap => "bitmap",
    }))
    .bind(subtitle.map(|details| bool_to_int(details.is_hearing_impaired)))
    .bind(bool_to_int(
        subtitle.is_some_and(|details| details.is_external),
    ))
    .bind(subtitle.and_then(|details| {
        details
            .external_relative_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
    }))
    .bind(subtitle.and_then(|details| details.downloaded_file.as_deref()))
    .bind(video.and_then(|details| details.margins).map(|it| it.top))
    .bind(
        video
            .and_then(|details| details.margins)
            .map(|it| it.bottom),
    )
    .bind(video.and_then(|details| details.margins).map(|it| it.left))
    .bind(video.and_then(|details| details.margins).map(|it| it.right))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Edges a film says are not part of its picture, when it says so.
///
/// Absent unless one of them takes something off: a file described before this
/// was read leaves the four empty, and a file that really has no margins says
/// the same thing, so the two need not be told apart.
fn margins_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Option<Margins>> {
    let edge = |name: &str| -> Result<i32> {
        Ok(row.try_get::<Option<i32>, _>(name)?.unwrap_or_default())
    };
    let margins = Margins {
        top: edge("margin_top")?,
        bottom: edge("margin_bottom")?,
        left: edge("margin_left")?,
        right: edge("margin_right")?,
    };
    Ok((!margins.are_nothing()).then_some(margins))
}

fn track_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Track> {
    let kind_text: String = row.try_get("kind")?;
    let codec: String = row.try_get("codec")?;

    let kind = match kind_text.as_str() {
        "video" => TrackKind::Video(VideoDetails {
            codec,
            profile: row.try_get("profile")?,
            level: row.try_get("level")?,
            width: row.try_get::<Option<i32>, _>("width")?.unwrap_or_default(),
            height: row.try_get::<Option<i32>, _>("height")?.unwrap_or_default(),
            margins: margins_from_row(row)?,
            aspect_ratio: row.try_get("aspect_ratio")?,
            is_interlaced: row
                .try_get::<Option<i64>, _>("is_interlaced")?
                .map(int_to_bool)
                .unwrap_or_default(),
            frame_rate: row.try_get("frame_rate")?,
            bitrate: row.try_get("bitrate")?,
            pixel_format: row.try_get("pixel_format")?,
            reference_frames: row.try_get("reference_frames")?,
            color: ColorInfo {
                primaries: row.try_get("color_primaries")?,
                space: row.try_get("color_space")?,
                transfer: row.try_get("color_transfer")?,
                bit_depth: row.try_get("bit_depth")?,
            },
            hdr: hdr_from_row(row)?,
        }),
        "audio" => TrackKind::Audio(AudioDetails {
            codec,
            profile: row.try_get("profile")?,
            channels: row
                .try_get::<Option<i32>, _>("channels")?
                .unwrap_or_default(),
            channel_layout: row.try_get("channel_layout")?,
            sample_rate: row.try_get("sample_rate")?,
            bit_depth: row.try_get("bit_depth")?,
            bitrate: row.try_get("bitrate")?,
            loudness: Loudness {
                integrated_lufs: row.try_get("loudness_integrated_lufs")?,
                true_peak_dbfs: row.try_get("loudness_true_peak_dbfs")?,
                range_lu: row.try_get("loudness_range_lu")?,
            },
        }),
        "subtitle" => TrackKind::Subtitle(SubtitleDetails {
            codec,
            // An unknown layout reads as pictures, the cautious side: taking
            // pictures for text shows a viewer nothing at all.
            layout: match row
                .try_get::<Option<String>, _>("subtitle_layout")?
                .as_deref()
            {
                Some("text") => SubtitleLayout::Text,
                _ => SubtitleLayout::Bitmap,
            },
            is_hearing_impaired: row
                .try_get::<Option<i64>, _>("is_hearing_impaired")?
                .map(int_to_bool)
                .unwrap_or_default(),
            is_external: int_to_bool(row.try_get("is_external")?),
            external_relative_path: row
                .try_get::<Option<String>, _>("external_relative_path")?
                .map(PathBuf::from),
            downloaded_file: row.try_get("downloaded_file")?,
        }),
        other => {
            return Err(DatabaseError::Corrupt(format!(
                "track kind '{other}' is unknown"
            )))
        }
    };

    Ok(Track {
        id: parse_id(&row.try_get::<String, _>("id")?)?,
        source_id: parse_id(&row.try_get::<String, _>("source_id")?)?,
        stream_index: row.try_get("stream_index")?,
        language: row.try_get("language")?,
        title: row.try_get("title")?,
        is_default: int_to_bool(row.try_get("is_default")?),
        is_forced: int_to_bool(row.try_get("is_forced")?),
        kind,
    })
}

fn hdr_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Option<HdrFormat>> {
    Ok(
        match row.try_get::<Option<String>, _>("hdr_format")?.as_deref() {
            Some("hdr10") => Some(HdrFormat::Hdr10),
            Some("hlg") => Some(HdrFormat::Hlg),
            Some("dolby_vision") => Some(HdrFormat::DolbyVision {
                profile: row.try_get("dolby_vision_profile")?,
            }),
            _ => None,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::id::TrackId;

    use crate::catalogue::tests::{library, video_track, work_with_source};

    fn audio_track(source_id: MediaSourceId) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 1,
            language: Some("fre".to_string()),
            title: Some("VFF".to_string()),
            is_default: true,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: "eac3".to_string(),
                profile: None,
                channels: 6,
                channel_layout: Some("5.1".to_string()),
                sample_rate: Some(48_000),
                bit_depth: Some(24),
                bitrate: Some(768_000),
                loudness: Loudness {
                    integrated_lufs: Some(-23.4),
                    true_peak_dbfs: Some(-1.2),
                    range_lu: Some(7.5),
                },
            }),
        }
    }

    fn subtitle_track(source_id: MediaSourceId) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 2,
            language: Some("fre".to_string()),
            title: None,
            is_default: false,
            is_forced: true,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: "hdmv_pgs_subtitle".to_string(),
                layout: SubtitleLayout::Bitmap,
                is_hearing_impaired: true,
                is_external: false,
                external_relative_path: None,
                downloaded_file: None,
            }),
        }
    }

    #[tokio::test]
    async fn every_stream_survives_a_round_trip_through_storage() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        let tracks = vec![
            video_track(source_id),
            audio_track(source_id),
            subtitle_track(source_id),
        ];
        database
            .store_analysis(
                source_id,
                &SourceAnalysis {
                    container: Some("matroska,webm".to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: Some(48_000_000),
                },
                &tracks,
                &[],
            )
            .await
            .expect("analysis stored");

        let stored = database
            .tracks_of_source(source_id)
            .await
            .expect("tracks read");
        assert_eq!(stored, tracks);
    }

    #[tokio::test]
    async fn the_wide_gamut_flavour_is_kept_because_the_playback_decision_rests_on_it() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        database
            .store_analysis(
                source_id,
                &SourceAnalysis::default(),
                &[video_track(source_id)],
                &[],
            )
            .await
            .expect("analysis stored");

        let stored = database
            .tracks_of_source(source_id)
            .await
            .expect("tracks read");
        let TrackKind::Video(details) = &stored[0].kind else {
            panic!("the first stream is the video one");
        };
        assert_eq!(
            details.hdr,
            Some(HdrFormat::DolbyVision { profile: Some(5) })
        );
        assert!(
            details
                .hdr
                .expect("present")
                .is_incompatible_without_conversion(),
            "the profile has to survive storage, it is what forbids playing the file untouched"
        );
        assert_eq!(details.color.transfer.as_deref(), Some("smpte2084"));
        assert_eq!(details.color.bit_depth, Some(10));
    }

    #[tokio::test]
    async fn every_wide_gamut_flavour_is_read_back_as_the_one_that_was_written() {
        // Each one calls for a different answer at playback time, and a
        // flavour that comes back as none would have the file played untouched
        // and shown washed out.
        let (database, library_id, root_id) = library().await;

        for flavour in [
            HdrFormat::Hdr10,
            HdrFormat::Hlg,
            HdrFormat::DolbyVision { profile: Some(8) },
        ] {
            let (_, source_id) = work_with_source(
                &database,
                library_id,
                root_id,
                &format!("Winter.Signal.{flavour:?}.mkv"),
            )
            .await;

            let mut track = video_track(source_id);
            if let TrackKind::Video(details) = &mut track.kind {
                details.hdr = Some(flavour);
            }
            database
                .store_analysis(source_id, &SourceAnalysis::default(), &[track], &[])
                .await
                .expect("analysis stored");

            let stored = database
                .tracks_of_source(source_id)
                .await
                .expect("tracks read");
            let TrackKind::Video(details) = &stored[0].kind else {
                panic!("the first stream is the video one");
            };
            assert_eq!(details.hdr, Some(flavour), "{flavour:?}");
        }
    }

    #[tokio::test]
    async fn an_ordinary_picture_carries_no_wide_gamut_flavour() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Amber.Field.2020.mkv").await;

        let mut track = video_track(source_id);
        if let TrackKind::Video(details) = &mut track.kind {
            details.hdr = None;
        }
        database
            .store_analysis(source_id, &SourceAnalysis::default(), &[track], &[])
            .await
            .expect("analysis stored");

        let stored = database
            .tracks_of_source(source_id)
            .await
            .expect("tracks read");
        let TrackKind::Video(details) = &stored[0].kind else {
            panic!("the first stream is the video one");
        };
        assert_eq!(details.hdr, None);
    }

    #[tokio::test]
    async fn analysing_the_same_file_twice_replaces_its_streams_rather_than_piling_them_up() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        for _ in 0..3 {
            database
                .store_analysis(
                    source_id,
                    &SourceAnalysis::default(),
                    &[video_track(source_id), audio_track(source_id)],
                    &[Chapter {
                        ordinal: 1,
                        start: Millis::new(0),
                        title: Some("Opening".to_string()),
                        thumbnail_path: None,
                    }],
                )
                .await
                .expect("analysis stored");
        }

        assert_eq!(
            database
                .tracks_of_source(source_id)
                .await
                .expect("read")
                .len(),
            2
        );
        assert_eq!(
            database
                .chapters_of_source(source_id)
                .await
                .expect("read")
                .len(),
            1
        );
    }

    fn external_subtitle(source_id: MediaSourceId) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 0,
            language: Some("fre".to_string()),
            title: None,
            is_default: false,
            is_forced: true,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: "subrip".to_string(),
                layout: SubtitleLayout::Text,
                is_hearing_impaired: false,
                is_external: true,
                external_relative_path: Some(PathBuf::from("Quiet.Harbour.2019.fr.forced.srt")),
                downloaded_file: None,
            }),
        }
    }

    #[tokio::test]
    async fn a_downloaded_subtitle_outlives_scans_and_analyses_until_taken_away() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        let mut downloaded = external_subtitle(source_id);
        if let TrackKind::Subtitle(details) = &mut downloaded.kind {
            details.external_relative_path = None;
            details.downloaded_file = Some("downloaded.srt".to_string());
        }
        database
            .add_downloaded_subtitle(source_id, &downloaded)
            .await
            .expect("added");
        assert_eq!(
            database.downloaded_subtitle_files().await.expect("read"),
            vec!["downloaded.srt".to_string()]
        );

        database
            .store_external_subtitles(source_id, &[])
            .await
            .expect("a scan found nothing next to the film");
        database
            .store_analysis(source_id, &SourceAnalysis::default(), &[video_track(source_id)], &[])
            .await
            .expect("analysed again");
        assert!(
            database
                .sources_with_external_subtitles(root_id)
                .await
                .expect("read")
                .is_empty(),
            "a downloaded subtitle is not one a scan looks after"
        );
        let stored = database.tracks_of_source(source_id).await.expect("read");
        assert!(stored.iter().any(|track| matches!(
            &track.kind,
            TrackKind::Subtitle(details) if details.downloaded_file.as_deref() == Some("downloaded.srt")
        )));

        assert_eq!(
            database
                .remove_downloaded_subtitle(source_id, stored[0].id)
                .await
                .expect("refused"),
            None,
            "a track that was not downloaded is not taken away here"
        );
        assert_eq!(
            database
                .remove_downloaded_subtitle(source_id, downloaded.id)
                .await
                .expect("taken away")
                .as_deref(),
            Some("downloaded.srt")
        );
        assert_eq!(database.tracks_of_source(source_id).await.expect("read").len(), 1);
        assert!(database.downloaded_subtitle_files().await.expect("read").is_empty());
    }

    #[tokio::test]
    async fn a_subtitle_in_its_own_file_survives_the_analysis_of_the_film() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        database
            .store_external_subtitles(source_id, &[external_subtitle(source_id)])
            .await
            .expect("subtitle stored");
        database
            .store_analysis(
                source_id,
                &SourceAnalysis::default(),
                &[video_track(source_id), subtitle_track(source_id)],
                &[],
            )
            .await
            .expect("analysis stored");

        let stored = database
            .tracks_of_source(source_id)
            .await
            .expect("tracks read");
        assert_eq!(stored.len(), 3, "the file next to the film is a track too");
        let external: Vec<&Track> = stored
            .iter()
            .filter(
                |track| matches!(&track.kind, TrackKind::Subtitle(details) if details.is_external),
            )
            .collect();
        assert_eq!(external.len(), 1);
        assert!(external[0].is_forced);
        assert_eq!(external[0].language.as_deref(), Some("fre"));

        // Text or pictures is what decides whether showing this subtitle costs
        // a full rebuild of the picture, so it has to survive storage.
        let layouts: Vec<SubtitleLayout> = stored
            .iter()
            .filter_map(|track| match &track.kind {
                TrackKind::Subtitle(details) => Some(details.layout),
                _ => None,
            })
            .collect();
        assert!(layouts.contains(&SubtitleLayout::Text));
        assert!(layouts.contains(&SubtitleLayout::Bitmap));
    }

    #[tokio::test]
    async fn the_films_carrying_a_subtitle_of_their_own_are_listed_for_the_scan() {
        // A scan asks for this so it rewrites only what changed. An answer of
        // nothing would make every scan rewrite every file in the library.
        let (database, library_id, root_id) = library().await;
        let (_, with_one) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        let (_, without) =
            work_with_source(&database, library_id, root_id, "Amber.Field.2020.mkv").await;

        database
            .store_external_subtitles(with_one, &[external_subtitle(with_one)])
            .await
            .expect("subtitle stored");
        // A subtitle inside the film is not a file beside it.
        database
            .store_analysis(
                without,
                &SourceAnalysis::default(),
                &[subtitle_track(without)],
                &[],
            )
            .await
            .expect("analysis stored");

        assert_eq!(
            database
                .sources_with_external_subtitles(root_id)
                .await
                .expect("read"),
            vec![with_one]
        );
    }

    #[tokio::test]
    async fn subtitles_in_their_own_files_are_replaced_rather_than_piled_up() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        for _ in 0..3 {
            database
                .store_external_subtitles(source_id, &[external_subtitle(source_id)])
                .await
                .expect("subtitle stored");
        }
        assert_eq!(
            database
                .tracks_of_source(source_id)
                .await
                .expect("read")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn a_replaced_film_keeps_the_subtitles_that_live_beside_it() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        database
            .store_external_subtitles(source_id, &[external_subtitle(source_id)])
            .await
            .expect("subtitle stored");
        database
            .store_analysis(
                source_id,
                &SourceAnalysis::default(),
                &[video_track(source_id)],
                &[],
            )
            .await
            .expect("analysis stored");

        database
            .refresh_source_identity(source_id, 2_000, now())
            .await
            .expect("identity refreshed");

        let stored = database.tracks_of_source(source_id).await.expect("read");
        assert_eq!(
            stored.len(),
            1,
            "the streams of the old copy go, the file next to it stays"
        );
        assert!(
            matches!(&stored[0].kind, TrackKind::Subtitle(details) if details.is_external),
            "a subtitle file describes itself, not the copy that was replaced"
        );
    }

    #[tokio::test]
    async fn only_the_files_still_waiting_for_an_analysis_are_handed_to_it() {
        let (database, library_id, root_id) = library().await;
        let (_, waiting) =
            work_with_source(&database, library_id, root_id, "Amber.Field.2020.mkv").await;
        let (_, already_done) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        let (_, gone) =
            work_with_source(&database, library_id, root_id, "Winter.Signal.2021.mkv").await;

        database
            .store_analysis(
                already_done,
                &SourceAnalysis {
                    container: Some("matroska,webm".to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: None,
                },
                &[video_track(already_done)],
                &[],
            )
            .await
            .expect("analysis stored");
        database.mark_source_missing(gone).await.expect("marked");

        let waiting_list = database
            .unanalysed_sources_of_root(root_id)
            .await
            .expect("read");
        assert_eq!(
            waiting_list
                .iter()
                .map(|source| source.id)
                .collect::<Vec<_>>(),
            vec![waiting],
            "a file already looked at is not looked at again, and one that is \
             not on the disk would only fail"
        );
    }
}
