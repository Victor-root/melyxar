//! Works, the files behind them, and what an analysis found inside.
//!
//! Two rules shape everything here. A work outlives the file that revealed it,
//! so replacing a copy with a better one keeps the watch history. And a file
//! that is no longer on disk is marked absent rather than deleted, so that an
//! unplugged disk is a bad evening rather than a lost library.

use std::path::{Path, PathBuf};

use melyxar_core::id::{ChapterId, ExtraVideoId, LibraryId, LibraryRootId, MediaSourceId, WorkId};
use melyxar_core::media::{
    AudioDetails, Chapter, ColorInfo, HdrFormat, Loudness, SubtitleDetails, SubtitleLayout, Track,
    TrackKind, VideoDetails,
};
use melyxar_core::time::{now, Millis, Timestamp};
use melyxar_core::work::{IdentificationState, Work, WorkKind};
use sqlx::Row;

use crate::convert::{
    bool_to_int, int_to_bool, parse_optional_timestamp, parse_timestamp, timestamp_to_text,
};
use crate::{Database, DatabaseError, Result};

/// A file as it is recorded, reduced to what a scan compares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSource {
    pub id: MediaSourceId,
    pub work_id: WorkId,
    pub relative_path: PathBuf,
    pub size_bytes: i64,
    pub modified_at: Timestamp,
    /// Set while the file is not on disk. A file that comes back keeps its
    /// identifier, and with it everything attached to it.
    pub missing_since: Option<Timestamp>,
}

/// What an analysis found about the file as a whole.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceAnalysis {
    pub container: Option<String>,
    pub duration: Option<Millis>,
    pub overall_bitrate: Option<i64>,
}

/// A video that belongs to a work without being the work itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalExtraVideo {
    pub kind: String,
    pub name: Option<String>,
    pub root_id: LibraryRootId,
    pub relative_path: PathBuf,
}

impl Database {
    /// Creates a work. Nothing is looked up yet, which is what `pending` says.
    pub async fn create_work(
        &self,
        library_id: LibraryId,
        kind: WorkKind,
        title: &str,
        sort_title: &str,
        release_year: Option<i32>,
    ) -> Result<Work> {
        let id = WorkId::new();
        let moment = now();
        let timestamp = timestamp_to_text(moment);

        sqlx::query(
            "INSERT INTO works (id, library_id, kind, title, sort_title, release_year, added_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(library_id.to_db_string())
        .bind(kind.as_str())
        .bind(title)
        .bind(sort_title)
        .bind(release_year)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(self.writer())
        .await?;

        Ok(Work {
            id,
            library_id,
            parent_id: None,
            kind,
            title: title.to_string(),
            sort_title: sort_title.to_string(),
            release_year,
            identification: IdentificationState::Pending,
            dominant_color: None,
            added_at: moment,
            updated_at: moment,
        })
    }

    /// One work by identifier.
    pub async fn work(&self, id: WorkId) -> Result<Option<Work>> {
        let row = sqlx::query(
            "SELECT id, library_id, parent_id, kind, title, sort_title, release_year,
                    identification, dominant_color, added_at, updated_at
             FROM works WHERE id = ?",
        )
        .bind(id.to_db_string())
        .fetch_optional(self.reader())
        .await?;

        row.map(|row| work_from_row(&row)).transpose()
    }

    /// Works of a library, newest first, which is the order the home page uses.
    pub async fn recent_works(&self, library_id: LibraryId, limit: i64) -> Result<Vec<Work>> {
        let rows = sqlx::query(
            "SELECT id, library_id, parent_id, kind, title, sort_title, release_year,
                    identification, dominant_color, added_at, updated_at
             FROM works WHERE library_id = ? ORDER BY added_at DESC LIMIT ?",
        )
        .bind(library_id.to_db_string())
        .bind(limit)
        .fetch_all(self.reader())
        .await?;

        rows.iter().map(work_from_row).collect()
    }

    /// How many works a library holds.
    pub async fn count_works(&self, library_id: LibraryId) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT count(*) FROM works WHERE library_id = ?")
            .bind(library_id.to_db_string())
            .fetch_one(self.reader())
            .await?;
        Ok(row.0)
    }

    /// Removes a work along with everything hanging from it.
    ///
    /// Only ever called by the caller that created a work and then found no
    /// file to attach to it. A scan never reaches for this.
    pub async fn delete_work(&self, id: WorkId) -> Result<()> {
        sqlx::query("DELETE FROM works WHERE id = ?")
            .bind(id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Every file recorded under one root, in path order.
    ///
    /// This is the side of the comparison a scan starts from, so it stays as
    /// small as the comparison needs.
    pub async fn sources_of_root(&self, root_id: LibraryRootId) -> Result<Vec<StoredSource>> {
        let rows = sqlx::query(
            "SELECT id, work_id, relative_path, size_bytes, modified_at, missing_since
             FROM media_sources WHERE root_id = ? ORDER BY relative_path",
        )
        .bind(root_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        let mut sources = Vec::with_capacity(rows.len());
        for row in rows {
            sources.push(StoredSource {
                id: parse_id(&row.try_get::<String, _>("id")?)?,
                work_id: parse_id(&row.try_get::<String, _>("work_id")?)?,
                relative_path: PathBuf::from(row.try_get::<String, _>("relative_path")?),
                size_bytes: row.try_get("size_bytes")?,
                modified_at: parse_timestamp(&row.try_get::<String, _>("modified_at")?)?,
                missing_since: parse_optional_timestamp(
                    row.try_get::<Option<String>, _>("missing_since")?
                        .as_deref(),
                )?,
            });
        }
        Ok(sources)
    }

    /// Records a file found by a scan.
    pub async fn insert_source(
        &self,
        work_id: WorkId,
        root_id: LibraryRootId,
        relative_path: &Path,
        size_bytes: i64,
        modified_at: Timestamp,
    ) -> Result<MediaSourceId> {
        let id = MediaSourceId::new();
        sqlx::query(
            "INSERT INTO media_sources
                (id, work_id, root_id, relative_path, size_bytes, modified_at, added_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(work_id.to_db_string())
        .bind(root_id.to_db_string())
        .bind(relative_path.to_string_lossy().as_ref())
        .bind(size_bytes)
        .bind(timestamp_to_text(modified_at))
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(id)
    }

    /// Records that a file changed on disk, which also clears any absence.
    ///
    /// The analysis it carried is dropped at the same time: it describes a
    /// file that no longer exists, and keeping it would mean serving the
    /// tracks of one copy while playing another.
    pub async fn refresh_source_identity(
        &self,
        id: MediaSourceId,
        size_bytes: i64,
        modified_at: Timestamp,
    ) -> Result<()> {
        let mut transaction = self.begin().await?;
        sqlx::query(
            "UPDATE media_sources
             SET size_bytes = ?, modified_at = ?, missing_since = NULL, analysed_at = NULL,
                 container = NULL, duration_ms = NULL, overall_bitrate = NULL
             WHERE id = ?",
        )
        .bind(size_bytes)
        .bind(timestamp_to_text(modified_at))
        .bind(id.to_db_string())
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM tracks WHERE source_id = ?")
            .bind(id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM chapters WHERE source_id = ?")
            .bind(id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Marks a file absent. Never a deletion: a disconnected disk must not
    /// cost a library.
    pub async fn mark_source_missing(&self, id: MediaSourceId) -> Result<()> {
        sqlx::query(
            "UPDATE media_sources SET missing_since = ? WHERE id = ? AND missing_since IS NULL",
        )
        .bind(timestamp_to_text(now()))
        .bind(id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Marks a file present again, keeping its identifier and its history.
    pub async fn mark_source_present(&self, id: MediaSourceId) -> Result<()> {
        sqlx::query("UPDATE media_sources SET missing_since = NULL WHERE id = ?")
            .bind(id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
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

        sqlx::query(
            "UPDATE media_sources
             SET container = ?, duration_ms = ?, overall_bitrate = ?, analysed_at = ?
             WHERE id = ?",
        )
        .bind(analysis.container.as_deref())
        .bind(analysis.duration.map(Millis::get))
        .bind(analysis.overall_bitrate)
        .bind(timestamp_to_text(now()))
        .bind(source_id.to_db_string())
        .execute(&mut *transaction)
        .await?;

        sqlx::query("DELETE FROM tracks WHERE source_id = ?")
            .bind(source_id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM chapters WHERE source_id = ?")
            .bind(source_id.to_db_string())
            .execute(&mut *transaction)
            .await?;

        for track in tracks {
            insert_track(&mut transaction, source_id, track).await?;
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
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(())
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

    /// Attaches a video sitting next to the work, such as a trailer.
    ///
    /// Replaces the entry at the same path rather than piling copies up, so a
    /// scan can run as often as it likes.
    pub async fn store_local_extra_video(
        &self,
        work_id: WorkId,
        extra: &LocalExtraVideo,
    ) -> Result<()> {
        let mut transaction = self.begin().await?;
        sqlx::query(
            "DELETE FROM extra_videos WHERE work_id = ? AND root_id = ? AND relative_path = ?",
        )
        .bind(work_id.to_db_string())
        .bind(extra.root_id.to_db_string())
        .bind(extra.relative_path.to_string_lossy().as_ref())
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO extra_videos (id, work_id, kind, name, root_id, relative_path, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(ExtraVideoId::new().to_db_string())
        .bind(work_id.to_db_string())
        .bind(&extra.kind)
        .bind(extra.name.as_deref())
        .bind(extra.root_id.to_db_string())
        .bind(extra.relative_path.to_string_lossy().as_ref())
        .bind(timestamp_to_text(now()))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Videos attached to a work, such as its trailers.
    pub async fn extra_videos_of_work(&self, work_id: WorkId) -> Result<Vec<LocalExtraVideo>> {
        let rows = sqlx::query(
            "SELECT kind, name, root_id, relative_path FROM extra_videos
             WHERE work_id = ? AND relative_path IS NOT NULL ORDER BY kind, relative_path",
        )
        .bind(work_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        let mut extras = Vec::with_capacity(rows.len());
        for row in rows {
            extras.push(LocalExtraVideo {
                kind: row.try_get("kind")?,
                name: row.try_get("name")?,
                root_id: parse_id(&row.try_get::<String, _>("root_id")?)?,
                relative_path: PathBuf::from(row.try_get::<String, _>("relative_path")?),
            });
        }
        Ok(extras)
    }
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
            subtitle_layout, is_hearing_impaired, is_external, external_relative_path
         ) VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?,
            ?, ?,
            ?, ?, ?,
            ?, ?, ?,
            ?, ?, ?, ?
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
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn work_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Work> {
    let kind_text: String = row.try_get("kind")?;
    let identification_text: String = row.try_get("identification")?;
    Ok(Work {
        id: parse_id(&row.try_get::<String, _>("id")?)?,
        library_id: parse_id(&row.try_get::<String, _>("library_id")?)?,
        parent_id: row
            .try_get::<Option<String>, _>("parent_id")?
            .map(|value| parse_id(&value))
            .transpose()?,
        kind: WorkKind::parse(&kind_text)
            .ok_or_else(|| DatabaseError::Corrupt(format!("work kind '{kind_text}' is unknown")))?,
        title: row.try_get("title")?,
        sort_title: row.try_get("sort_title")?,
        release_year: row.try_get("release_year")?,
        identification: IdentificationState::parse(&identification_text).ok_or_else(|| {
            DatabaseError::Corrupt(format!(
                "identification state '{identification_text}' is unknown"
            ))
        })?,
        dominant_color: row.try_get("dominant_color")?,
        added_at: parse_timestamp(&row.try_get::<String, _>("added_at")?)?,
        updated_at: parse_timestamp(&row.try_get::<String, _>("updated_at")?)?,
    })
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

fn parse_id<T: std::str::FromStr>(value: &str) -> Result<T> {
    value
        .parse()
        .map_err(|_| DatabaseError::Corrupt(format!("identifier '{value}' is malformed")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::id::TrackId;
    use melyxar_core::library::LibraryKind;

    async fn library() -> (Database, LibraryId, LibraryRootId) {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");
        let root_id = library.roots[0].id;
        (database, library.id, root_id)
    }

    fn video_track(source_id: MediaSourceId) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 0,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Video(VideoDetails {
                codec: "hevc".to_string(),
                profile: Some("Main 10".to_string()),
                level: Some(153),
                width: 3840,
                height: 2160,
                aspect_ratio: Some("16:9".to_string()),
                is_interlaced: false,
                frame_rate: Some(23.976),
                bitrate: Some(45_000_000),
                pixel_format: Some("yuv420p10le".to_string()),
                reference_frames: Some(4),
                color: ColorInfo {
                    primaries: Some("bt2020".to_string()),
                    space: Some("bt2020nc".to_string()),
                    transfer: Some("smpte2084".to_string()),
                    bit_depth: Some(10),
                },
                hdr: Some(HdrFormat::DolbyVision { profile: Some(5) }),
            }),
        }
    }

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
            }),
        }
    }

    async fn work_with_source(
        database: &Database,
        library_id: LibraryId,
        root_id: LibraryRootId,
        relative: &str,
    ) -> (WorkId, MediaSourceId) {
        let work = database
            .create_work(
                library_id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        let source = database
            .insert_source(work.id, root_id, Path::new(relative), 1_000, now())
            .await
            .expect("source recorded");
        (work.id, source)
    }

    #[tokio::test]
    async fn a_work_starts_out_waiting_to_be_identified() {
        let (database, library_id, _) = library().await;
        let work = database
            .create_work(
                library_id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");

        assert_eq!(work.identification, IdentificationState::Pending);
        assert!(work.identification.may_be_looked_up_again());

        let reloaded = database
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(reloaded, work);
        assert_eq!(reloaded.kind, WorkKind::Movie);
        assert!(reloaded.kind.is_playable());
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

    #[tokio::test]
    async fn a_file_that_is_gone_is_marked_absent_and_comes_back_with_its_history() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        database
            .mark_source_missing(source_id)
            .await
            .expect("marked");
        let stored = database.sources_of_root(root_id).await.expect("read");
        assert_eq!(stored.len(), 1, "a scan never deletes a row");
        assert!(stored[0].missing_since.is_some());

        database
            .mark_source_present(source_id)
            .await
            .expect("marked");
        let back = database.sources_of_root(root_id).await.expect("read");
        assert_eq!(
            back[0].id, source_id,
            "the identifier is what carries the history"
        );
        assert!(back[0].missing_since.is_none());
    }

    #[tokio::test]
    async fn a_replaced_file_drops_the_analysis_of_the_copy_that_is_gone() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        database
            .store_analysis(
                source_id,
                &SourceAnalysis {
                    container: Some("matroska,webm".to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: None,
                },
                &[video_track(source_id)],
                &[],
            )
            .await
            .expect("analysis stored");

        database
            .refresh_source_identity(source_id, 2_000, now())
            .await
            .expect("identity refreshed");

        assert!(
            database
                .tracks_of_source(source_id)
                .await
                .expect("read")
                .is_empty(),
            "serving the streams of one copy while playing another is a defect, not a shortcut"
        );
        let stored = database.sources_of_root(root_id).await.expect("read");
        assert_eq!(stored[0].size_bytes, 2_000);
    }

    #[tokio::test]
    async fn stored_files_come_back_in_path_order_so_two_runs_can_be_compared() {
        let (database, library_id, root_id) = library().await;
        for name in ["c.mkv", "a.mkv", "b.mkv"] {
            work_with_source(&database, library_id, root_id, name).await;
        }
        let stored = database.sources_of_root(root_id).await.expect("read");
        let paths: Vec<String> = stored
            .iter()
            .map(|source| source.relative_path.to_string_lossy().into_owned())
            .collect();
        assert_eq!(paths, vec!["a.mkv", "b.mkv", "c.mkv"]);
    }

    #[tokio::test]
    async fn a_trailer_next_to_a_film_is_attached_to_it_without_piling_up() {
        let (database, library_id, root_id) = library().await;
        let (work_id, _) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        let extra = LocalExtraVideo {
            kind: "trailer".to_string(),
            name: None,
            root_id,
            relative_path: PathBuf::from("Quiet.Harbour.2019-trailer.mkv"),
        };
        for _ in 0..2 {
            database
                .store_local_extra_video(work_id, &extra)
                .await
                .expect("trailer attached");
        }

        let stored = database.extra_videos_of_work(work_id).await.expect("read");
        assert_eq!(stored, vec![extra]);
    }

    #[tokio::test]
    async fn works_are_counted_and_listed_newest_first() {
        let (database, library_id, root_id) = library().await;
        for name in ["a.mkv", "b.mkv", "c.mkv"] {
            work_with_source(&database, library_id, root_id, name).await;
        }
        assert_eq!(database.count_works(library_id).await.expect("read"), 3);

        let recent = database.recent_works(library_id, 2).await.expect("read");
        assert_eq!(recent.len(), 2);
        assert!(recent[0].added_at >= recent[1].added_at);
    }

    #[tokio::test]
    async fn a_malformed_stored_kind_is_reported_rather_than_guessed() {
        let (database, library_id, _) = library().await;
        let work = database
            .create_work(
                library_id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                None,
            )
            .await
            .expect("work created");

        sqlx::query("UPDATE works SET kind = 'something_new' WHERE id = ?")
            .bind(work.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("value forced");

        assert!(matches!(
            database.work(work.id).await,
            Err(DatabaseError::Corrupt(_))
        ));
    }

    #[tokio::test]
    async fn removing_a_work_takes_its_files_and_streams_with_it() {
        let (database, library_id, root_id) = library().await;
        let (work_id, source_id) =
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

        database.delete_work(work_id).await.expect("work removed");

        assert!(database
            .sources_of_root(root_id)
            .await
            .expect("read")
            .is_empty());
        assert!(database
            .tracks_of_source(source_id)
            .await
            .expect("read")
            .is_empty());
    }
}
