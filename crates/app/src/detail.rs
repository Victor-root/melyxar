//! Everything one page about one work needs.
//!
//! Assembled here rather than left to the client to gather piece by piece: a
//! page that opens with eight requests opens eight times slower than one that
//! opens with one, and on a television that is the difference between a page
//! that appears and a page that assembles itself in front of you.

use melyxar_core::id::{MediaSourceId, PersonId, WorkId};
use melyxar_core::media::{Chapter, Track};
use melyxar_core::time::{Millis, Timestamp};
use melyxar_core::work::Work;
use melyxar_database::catalogue::{ChildWork, PlayableExtraVideo, SourceAnalysis};
use melyxar_database::images::StoredImage;

use crate::{AppState, Result};

/// A work with everything its page shows.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkDetail {
    pub work: Work,
    /// Title, tagline and synopsis in the language the library was asked for.
    pub tagline: Option<String>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub studios: Vec<String>,
    /// Who is credited, leads first, then the parts behind the camera.
    pub credits: Vec<Credit>,
    /// The saga this film belongs to, when it belongs to one.
    pub collection: Option<String>,
    pub images: Vec<StoredImage>,
    /// Every copy on disk, so a page can offer a choice between them.
    pub versions: Vec<Version>,
    /// Trailers, the local ones first since they play without leaving here.
    pub trailers: Vec<TrailerLink>,
    pub external_ids: Vec<(String, String)>,
    /// What hangs under this one, in order: the seasons of a series, the
    /// episodes of a season. Empty for anything met on its own.
    pub children: Vec<Child>,
    /// What this one hangs under, nearest first: an episode answers with its
    /// season and then its series. Empty for anything met on its own.
    ///
    /// Sent with the page rather than fetched by it, because the way back up
    /// is drawn before anything else and a page that asks for its own parent
    /// draws a heading that arrives late.
    pub ancestry: Vec<Ancestor>,
    /// The episode this viewer would watch next, on the page of a series or of
    /// a season. Absent when there is none left to watch, and for anything met
    /// on its own.
    pub carry_on_with: Option<CarryOn>,
}

/// The episode a page offers to play next, and where in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarryOn {
    pub id: WorkId,
    /// Which season and which episode it is, so a button can say so.
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub title: String,
    /// Whether this title says anything its number does not.
    pub has_own_name: bool,
    /// The file it would be played from. Absent would mean nothing to play,
    /// and such an episode is never offered.
    pub source_id: Option<MediaSourceId>,
}

/// One work hanging under this one, with what its card shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Child {
    pub work: ChildWork,
    /// Every size of its picture, largest first.
    pub poster: Vec<StoredImage>,
}

/// One work this one hangs under, as a way back to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ancestor {
    pub id: WorkId,
    pub kind: melyxar_core::work::WorkKind,
    pub ordinal: Option<i32>,
    pub title: String,
}

/// One line of the credits, with the face shown next to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credit {
    pub person_id: PersonId,
    pub name: String,
    /// actor, director, writer, producer, composer.
    pub role: String,
    /// Who they played, for an actor.
    pub character: Option<String>,
    /// Every size of their face, largest first. Empty until it has been
    /// fetched, and for everyone the provider has no picture of.
    pub photo: Vec<StoredImage>,
}

/// One copy of a work on disk, with what it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Version {
    pub source_id: MediaSourceId,
    pub relative_path: String,
    /// The disk this copy lives on, by the name the configuration gives it.
    pub root_label: String,
    /// Where the file is, root included. Shown to whoever runs the server: on
    /// a page about a file, the first thing wanted when something is wrong
    /// with it is where it is. Never used to reach it: playback goes through
    /// the identifier, which is what keeps a path from being an address.
    pub path: String,
    /// When a scan first saw it, which is not when the file was made.
    pub added_at: Timestamp,
    pub size_bytes: i64,
    /// Set while the file is not on disk. A version that cannot be played is
    /// shown as such rather than offered and failing.
    pub missing_since: Option<Timestamp>,
    pub container: Option<String>,
    pub duration: Option<Millis>,
    pub overall_bitrate: Option<i64>,
    pub analysed: bool,
    pub tracks: Vec<Track>,
    pub chapters: Vec<Chapter>,
}

impl Version {
    /// A short line of the kind shown above a play button.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some((_, video)) = self.tracks.iter().find_map(|track| match &track.kind {
            melyxar_core::media::TrackKind::Video(details) => Some((track, details)),
            _ => None,
        }) {
            parts.push(video.summary());
        }

        let audio = self
            .tracks
            .iter()
            .filter(|track| matches!(track.kind, melyxar_core::media::TrackKind::Audio(_)))
            .count();
        if audio > 1 {
            parts.push(format!("{audio} audio"));
        }

        let subtitles = self
            .tracks
            .iter()
            .filter(|track| matches!(track.kind, melyxar_core::media::TrackKind::Subtitle(_)))
            .count();
        if subtitles > 0 {
            parts.push(format!("{subtitles} sub"));
        }
        parts.join(" · ")
    }
}

/// Where a trailer can be watched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrailerLink {
    pub name: Option<String>,
    /// Set for a file sitting next to the film, which plays from here.
    pub local: Option<PlayableExtraVideo>,
    /// Set for a link, which leaves this server.
    pub remote_url: Option<String>,
}

/// The episode a page offers to play next, ready to be drawn.
///
/// Whichever of a series the viewer has not watched comes first, and failing
/// that nothing: a series watched through is one to start again from its own
/// list rather than from a button that says "carry on".
async fn what_to_carry_on_with(
    state: &AppState,
    viewer: melyxar_core::id::UserId,
    series_id: WorkId,
) -> Result<Option<CarryOn>> {
    let database = state.database();
    let Some(episode) = database.where_to_resume(viewer, series_id).await? else {
        return Ok(None);
    };
    Ok(Some(as_carry_on(state, episode).await?))
}

/// One episode, ready for the button that offers it.
///
/// Written once because two questions end here: the one a series page asks and
/// the one an episode page asks. A button that says the season on one page and
/// not on the other would be the same button behaving differently.
async fn as_carry_on(state: &AppState, episode: melyxar_core::work::Work) -> Result<CarryOn> {
    let database = state.database();

    // The season it sits in, for a button that says which episode it means.
    let season = match episode.parent_id {
        Some(parent) => database
            .work(parent)
            .await?
            .and_then(|season| season.ordinal),
        None => None,
    };

    Ok(CarryOn {
        has_own_name: !crate::episodes::is_only_a_number(
            episode.kind,
            episode.ordinal,
            &episode.title,
        ),
        // The biggest copy, which is what pressing play without choosing means
        // everywhere else in this server.
        source_id: database
            .sources_of_work(episode.id)
            .await?
            .into_iter()
            .filter(|source| source.missing_since.is_none())
            .max_by_key(|source| source.size_bytes)
            .map(|source| source.id),
        id: episode.id,
        season,
        episode: episode.ordinal,
        title: episode.title,
    })
}

/// Reads everything one page shows about one work.
pub async fn work_detail(
    state: &AppState,
    viewer: melyxar_core::id::UserId,
    work_id: WorkId,
) -> Result<Option<WorkDetail>> {
    let database = state.database();
    let Some(work) = database.work(work_id).await? else {
        return Ok(None);
    };

    let language = database
        .list_libraries()
        .await?
        .into_iter()
        .find(|library| library.id == work.library_id)
        .map(|library| library.metadata_language)
        .unwrap_or_else(|| "fr".to_string());

    // The text of the language the library speaks, falling back to English,
    // which is what a provider answers with when it has nothing else.
    let texts = match database.work_translation(work_id, &language).await? {
        Some(texts) => Some(texts),
        None => database.work_translation(work_id, "en").await?,
    };

    let mut versions = Vec::new();
    for source in database.sources_of_work(work_id).await? {
        let (analysis, analysed_at) = database
            .source_details(source.id)
            .await?
            .unwrap_or((SourceAnalysis::default(), None));

        versions.push(Version {
            source_id: source.id,
            relative_path: source.relative_path.to_string_lossy().into_owned(),
            root_label: source.root_label.clone(),
            path: source
                .root_path
                .join(&source.relative_path)
                .to_string_lossy()
                .into_owned(),
            added_at: source.added_at,
            size_bytes: source.size_bytes,
            missing_since: source.missing_since,
            container: analysis.container,
            duration: analysis.duration,
            overall_bitrate: analysis.overall_bitrate,
            analysed: analysed_at.is_some(),
            tracks: database.tracks_of_source(source.id).await?,
            chapters: database.chapters_of_source(source.id).await?,
        });
    }

    // The biggest copy first: it is the one a viewer means when they press
    // play without choosing.
    versions.sort_by_key(|version| std::cmp::Reverse(version.size_bytes));

    let mut trailers: Vec<TrailerLink> = database
        .extra_videos_of_work(work_id)
        .await?
        .into_iter()
        .map(|extra| TrailerLink {
            name: extra.name.clone(),
            local: Some(extra),
            remote_url: None,
        })
        .collect();
    trailers.extend(
        database
            .remote_extra_videos_of_work(work_id)
            .await?
            .into_iter()
            .map(|(name, url)| TrailerLink {
                name: Some(name),
                local: None,
                remote_url: Some(url),
            }),
    );

    // The faces come back in one read, then each one joins its own name: a page
    // of eighteen credits must not cost eighteen round trips.
    let faces = database.credit_photos_of_work(work_id).await?;
    let credits = database
        .work_credits(work_id)
        .await?
        .into_iter()
        .map(|credit| {
            let owner = credit.person_id.to_db_string();
            Credit {
                photo: faces
                    .iter()
                    .filter(|image| image.owner_id == owner)
                    .cloned()
                    .collect(),
                person_id: credit.person_id,
                name: credit.name,
                role: credit.role,
                character: credit.character,
            }
        })
        .collect();

    // The pictures of a whole page of episodes in one read, then each one
    // joins its own card, exactly as the faces do above.
    let pictures = database.pictures_of_children(work_id).await?;
    let children = database
        .children_of(viewer, work_id)
        .await?
        .into_iter()
        .map(|child| {
            let owner = child.id.to_db_string();
            Child {
                poster: pictures
                    .iter()
                    .filter(|image| image.owner_id == owner)
                    .cloned()
                    .collect(),
                work: child,
            }
        })
        .collect();

    let ancestry = database
        .ancestry_of(work_id)
        .await?
        .into_iter()
        .map(|work| Ancestor {
            id: work.id,
            kind: work.kind,
            ordinal: work.ordinal,
            title: work.title,
        })
        .collect();

    // What a page offers to play next, from what it already knows. A series
    // and a season both answer for the whole series: somebody who opens season
    // one having watched it all means to carry on into season two, not to sit
    // on a button that starts again where they already are.
    let carry_on_with = match work.kind {
        melyxar_core::work::WorkKind::Series => {
            what_to_carry_on_with(state, viewer, work.id).await?
        }
        melyxar_core::work::WorkKind::Season => match work.parent_id {
            Some(series_id) => what_to_carry_on_with(state, viewer, series_id).await?,
            None => None,
        },
        // On an episode it is the one after this one, watched or not: somebody
        // at the end of an episode means the next one, not the next one they
        // happen to have missed.
        melyxar_core::work::WorkKind::Episode => {
            match database.next_episode_after(viewer, work.id).await? {
                Some(next) => Some(as_carry_on(state, next).await?),
                None => None,
            }
        }
        _ => None,
    };

    Ok(Some(WorkDetail {
        children,
        ancestry,
        carry_on_with,
        tagline: texts.as_ref().and_then(|(_, tagline, _)| tagline.clone()),
        overview: texts.as_ref().and_then(|(_, _, overview)| overview.clone()),
        genres: database.work_genres(work_id).await?,
        studios: database.work_studios(work_id).await?,
        credits,
        collection: database.work_collection(work_id).await?,
        images: database.images_of("work", &work_id.to_db_string()).await?,
        external_ids: database.work_external_ids(work_id).await?,
        versions,
        trailers,
        work,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::id::TrackId;
    use melyxar_core::media::{
        AudioDetails, ColorInfo, SubtitleDetails, SubtitleLayout, TrackKind, VideoDetails,
    };

    fn video(width: i32, height: i32) -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: 0,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Video(VideoDetails {
                codec: "hevc".to_string(),
                profile: None,
                level: None,
                width,
                height,
                margins: None,
                aspect_ratio: None,
                is_interlaced: false,
                frame_rate: None,
                bitrate: None,
                pixel_format: None,
                reference_frames: None,
                color: ColorInfo::default(),
                hdr: None,
            }),
        }
    }

    fn audio() -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: 1,
            language: Some("fre".to_string()),
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: "eac3".to_string(),
                profile: None,
                channels: 6,
                channel_layout: Some("5.1".to_string()),
                sample_rate: Some(48_000),
                bit_depth: None,
                bitrate: None,
                loudness: Default::default(),
            }),
        }
    }

    fn subtitle() -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: 2,
            language: Some("fre".to_string()),
            title: None,
            is_default: false,
            is_forced: false,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: "subrip".to_string(),
                layout: SubtitleLayout::Text,
                is_hearing_impaired: false,
                is_external: true,
                external_relative_path: None,
            }),
        }
    }

    fn version(tracks: Vec<Track>) -> Version {
        Version {
            source_id: MediaSourceId::new(),
            root_label: "disk-one".to_string(),
            path: "/mnt/disk-one/films/a.mkv".to_string(),
            added_at: melyxar_core::time::now(),
            relative_path: "Quiet.Harbour.2019.mkv".to_string(),
            size_bytes: 1_000,
            missing_since: None,
            container: Some("matroska,webm".to_string()),
            duration: Some(Millis::new(7_200_000)),
            overall_bitrate: None,
            analysed: true,
            tracks,
            chapters: Vec::new(),
        }
    }

    #[test]
    fn a_version_says_in_one_line_what_it_holds() {
        let summary = version(vec![video(3840, 2160), audio(), audio(), subtitle()]).summary();
        assert!(summary.contains("4K"), "{summary}");
        assert!(summary.contains("2 audio"), "{summary}");
        assert!(summary.contains("1 sub"), "{summary}");
    }

    #[test]
    fn a_single_soundtrack_is_not_worth_counting_out_loud() {
        let summary = version(vec![video(1920, 1080), audio()]).summary();
        assert!(summary.contains("1080p"), "{summary}");
        assert!(
            !summary.contains("audio"),
            "saying one soundtrack is saying nothing: {summary}"
        );
    }

    #[test]
    fn a_file_nothing_has_looked_at_yet_still_has_a_line() {
        let summary = version(Vec::new()).summary();
        assert!(summary.is_empty(), "nothing known means nothing claimed");
    }

    /// Two libraries speaking two languages, so that a page reading the wrong
    /// one is told apart from a page reading the right one.
    async fn two_libraries() -> (melyxar_database::Database, WorkId) {
        use melyxar_core::library::LibraryKind;
        use melyxar_core::work::WorkKind;
        use melyxar_database::metadata::IdentifiedWork;
        use std::path::PathBuf;

        let database = melyxar_database::Database::open_in_memory()
            .await
            .expect("database opens");
        database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");
        let english = database
            .create_library(
                "Shows",
                LibraryKind::Shows,
                "en",
                &[("disk-two".to_string(), PathBuf::from("/mnt/two/Shows"))],
            )
            .await
            .expect("library created");

        let work = database
            .create_work(
                english.id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");

        // The same film described twice, once per language, as a provider
        // answers when asked twice.
        for (language, overview) in [
            ("fr", "Un port, une nuit."),
            ("en", "A harbour, one night."),
        ] {
            database
                .apply_identification(
                    work.id,
                    &IdentifiedWork {
                        provider: "tmdb".to_string(),
                        external_id: "111".to_string(),
                        imdb_id: None,
                        language: language.to_string(),
                        title: "Quiet Harbour".to_string(),
                        sort_title: "quiet harbour".to_string(),
                        tagline: None,
                        overview: Some(overview.to_string()),
                        release_year: Some(2019),
                        runtime: None,
                        community_rating: None,
                        age_rating_label: None,
                        genres: Vec::new(),
                        studios: Vec::new(),
                        credits: Vec::new(),
                        collection: None,
                        trailers: Vec::new(),
                    },
                    false,
                )
                .await
                .expect("identification applied");
        }

        (database, work.id)
    }

    #[tokio::test]
    async fn a_page_speaks_the_language_of_the_library_the_film_is_in() {
        let (database, work_id) = two_libraries().await;
        let viewer = database
            .create_user("Viewer", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account created")
            .id;
        let state = AppState::new(melyxar_config::Config::default(), database, None, None);

        let detail = work_detail(&state, viewer, work_id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(
            detail.overview.as_deref(),
            Some("A harbour, one night."),
            "the film is in the library that speaks English, not in the other one"
        );
    }
}
