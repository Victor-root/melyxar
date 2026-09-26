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
use melyxar_database::browse::WorkCard;
use melyxar_database::catalogue::{PlayableExtraVideo, SourceAnalysis};
use melyxar_database::home::Alike;
use melyxar_database::images::StoredImage;

use crate::{AppState, Result};

/// One work hanging under another, as a page carries it.
pub use melyxar_database::catalogue::ChildWork;

/// How many works the row of alike ones holds: a few screens of it, and
/// never the whole genre.
const ROW_OF_ALIKE: i64 = 24;

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
    pub children: Vec<ChildWork>,
    /// Every episode of the season an episode belongs to, itself included, in
    /// order: the way from one to the next on its page and in the player.
    /// Empty for anything that is not an episode.
    pub siblings: Vec<ChildWork>,
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
    /// The episode before this one, so the player can offer to step back into
    /// it. Absent for anything that is not an episode, and for the first
    /// episode of a series.
    pub previous_episode: Option<CarryOn>,
    /// The photos before and after this one in its folder, for looking
    /// through them one by one. Both absent for anything but a photo.
    pub previous_photo: Option<WorkId>,
    pub next_photo: Option<WorkId>,
    /// This work as its card, with what the viewer has made of it: the page
    /// marks it seen or liked through the same card every row draws.
    pub card: Option<WorkCard>,
    /// Works like this one, by a genre they share. Only for a film and a
    /// series, which are what genres are written on.
    pub alike: Option<Alike>,
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

/// One work this one hangs under, as a way back to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ancestor {
    pub id: WorkId,
    pub kind: melyxar_core::work::WorkKind,
    pub ordinal: Option<i32>,
    pub title: String,
    /// The series' own mark, drawn as it draws its title. Empty for anything
    /// that is not a series: a season and an episode are never asked to
    /// illustrate their own title, only their series is.
    pub logo: Vec<StoredImage>,
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
    /// Only for an administrator, like the path below.
    pub root_label: Option<String>,
    /// Where the file is, root included. Shown to whoever runs the server: on
    /// a page about a file, the first thing wanted when something is wrong
    /// with it is where it is. Never used to reach it: playback goes through
    /// the identifier, which is what keeps a path from being an address.
    ///
    /// Absent for anybody else: where the server keeps its files is about
    /// the server, not about the film.
    pub path: Option<String>,
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
/// Where the viewer is in a series, or in one season of it, and failing that
/// nothing: a series watched through is one to start again from its own list
/// rather than from a button that says "carry on".
async fn what_to_carry_on_with(
    state: &AppState,
    viewer: melyxar_core::id::UserId,
    within: WorkId,
) -> Result<Option<CarryOn>> {
    let database = state.database();
    let Some(episode) = database.where_to_resume(viewer, within).await? else {
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
    who: &melyxar_core::user::User,
    work_id: WorkId,
) -> Result<Option<WorkDetail>> {
    let database = state.database();
    let Some(work) = database.work(work_id).await? else {
        return Ok(None);
    };
    // A work in a library this account was not granted is a work that is not
    // there, which is the same answer as a work nobody has: told apart, one
    // identifier after another would say what this server holds.
    if !who.permissions.may_access_library(work.library_id) {
        return Ok(None);
    }
    let viewer = who.id;

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

    let runs_the_server = who.permissions.is_administrator;
    let mut versions = Vec::new();
    for source in database.sources_of_work(work_id).await? {
        let (analysis, analysed_at) = database
            .source_details(source.id)
            .await?
            .unwrap_or((SourceAnalysis::default(), None));

        versions.push(Version {
            source_id: source.id,
            relative_path: source.relative_path.to_string_lossy().into_owned(),
            root_label: runs_the_server.then(|| source.root_label.clone()),
            path: runs_the_server.then(|| {
                source
                    .root_path
                    .join(&source.relative_path)
                    .to_string_lossy()
                    .into_owned()
            }),
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

    let children = database.children_of(viewer, work_id, &language).await?;
    let siblings = match (work.kind, work.parent_id) {
        (melyxar_core::work::WorkKind::Episode, Some(season)) => {
            database.children_of(viewer, season, &language).await?
        }
        _ => Vec::new(),
    };

    // Only a series is ever given a mark of its own: a season and an episode
    // are described by it rather than illustrated on their own, so asking for
    // their pictures would only ever come back with a poster nobody wants
    // here. Its backdrop is kept aside for the page below it.
    let mut ancestry = Vec::new();
    let mut series_backdrop = Vec::new();
    for up in database.ancestry_of(work_id).await? {
        let mut logo = Vec::new();
        if up.kind == melyxar_core::work::WorkKind::Series {
            for image in database.images_of("work", &up.id.to_db_string()).await? {
                match image.image_kind.as_str() {
                    "logo" => logo.push(image),
                    "backdrop" => series_backdrop.push(image),
                    _ => {}
                }
            }
        }
        ancestry.push(Ancestor {
            id: up.id,
            kind: up.kind,
            ordinal: up.ordinal,
            title: up.title,
            logo,
        });
    }

    // A season and an episode are almost never given a backdrop of their own
    // by the provider, and their page stood on a bare ground where the page
    // of their series has a picture. Theirs when they have one, their
    // series' otherwise.
    let mut images = database.images_of("work", &work_id.to_db_string()).await?;
    if !images.iter().any(|image| image.image_kind == "backdrop") {
        images.extend(series_backdrop);
    }

    // What a page offers to play next, from what it already knows. A season
    // answers for itself first: somebody on the page of season twenty means
    // season twenty, not the first episode of a series they never started
    // from the beginning. Once it is watched through, it answers for the
    // whole series, since somebody who opens season one having watched it
    // all means to carry on into season two rather than to sit on a button
    // that starts again where they already are.
    let carry_on_with = match work.kind {
        melyxar_core::work::WorkKind::Series => {
            what_to_carry_on_with(state, viewer, work.id).await?
        }
        melyxar_core::work::WorkKind::Season => {
            match what_to_carry_on_with(state, viewer, work.id).await? {
                Some(inside) => Some(inside),
                None => match work.parent_id {
                    Some(series_id) => what_to_carry_on_with(state, viewer, series_id).await?,
                    None => None,
                },
            }
        }
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

    // Only an episode is ever stepped back from: a series and a season have
    // nothing playing to step back out of.
    let previous_episode = match work.kind {
        melyxar_core::work::WorkKind::Episode => {
            match database.previous_episode_before(work.id).await? {
                Some(previous) => Some(as_carry_on(state, previous).await?),
                None => None,
            }
        }
        _ => None,
    };

    let (previous_photo, next_photo) = match work.kind {
        melyxar_core::work::WorkKind::Photo => database.neighbouring_photos(&work).await?,
        _ => (None, None),
    };

    let alike = match work.kind {
        melyxar_core::work::WorkKind::Movie | melyxar_core::work::WorkKind::Series => {
            let within = crate::reach::within(who);
            database
                .alike_by_genre(viewer, work_id, within.as_deref(), ROW_OF_ALIKE)
                .await?
        }
        _ => None,
    };

    Ok(Some(WorkDetail {
        card: database.card_of(viewer, work_id).await?,
        alike,
        previous_photo,
        next_photo,
        children,
        siblings,
        ancestry,
        carry_on_with,
        previous_episode,
        tagline: texts.as_ref().and_then(|(_, tagline, _)| tagline.clone()),
        overview: texts.as_ref().and_then(|(_, _, overview)| overview.clone()),
        genres: database.work_genres(work_id).await?,
        studios: database.work_studios(work_id).await?,
        credits,
        collection: database.work_collection(work_id).await?,
        images,
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
            root_label: Some("disk-one".to_string()),
            path: Some("/mnt/disk-one/films/a.mkv".to_string()),
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

        let detail = work_detail(&state, &crate::an_ordinary_account(viewer), work_id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(
            detail.overview.as_deref(),
            Some("A harbour, one night."),
            "the film is in the library that speaks English, not in the other one"
        );
    }

    #[tokio::test]
    async fn an_episode_climbs_up_to_its_series_mark() {
        use melyxar_core::work::WorkKind;
        use melyxar_database::images::StoredImage;
        use std::path::PathBuf;

        let database = melyxar_database::Database::open_in_memory()
            .await
            .expect("database opens");
        let library = database
            .create_library(
                "Shows",
                melyxar_core::library::LibraryKind::Shows,
                "en",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Shows"))],
            )
            .await
            .expect("library created");

        let series = database
            .create_work(
                library.id,
                WorkKind::Series,
                "Distant Signal",
                "distant signal",
                Some(2019),
            )
            .await
            .expect("series created");
        let season = database
            .create_child_work(
                library.id,
                series.id,
                1,
                WorkKind::Season,
                "Season 1",
                "season 1",
            )
            .await
            .expect("season created");
        let episode = database
            .create_child_work(
                library.id,
                season.id,
                1,
                WorkKind::Episode,
                "The Long Night",
                "the long night",
            )
            .await
            .expect("episode created");

        // The series has its own mark; a season is never asked to draw one.
        database
            .replace_images(
                "work",
                &series.id.to_db_string(),
                "logo",
                &[StoredImage {
                    owner_kind: "work".to_string(),
                    owner_id: series.id.to_db_string(),
                    image_kind: "logo".to_string(),
                    relative_path: "works/distant-signal/logo-340.webp".to_string(),
                    width: Some(340),
                    height: Some(120),
                    fingerprint: "abc123".to_string(),
                    dominant_color: None,
                }],
            )
            .await
            .expect("logo stored");
        database
            .replace_images(
                "work",
                &series.id.to_db_string(),
                "backdrop",
                &[StoredImage {
                    owner_kind: "work".to_string(),
                    owner_id: series.id.to_db_string(),
                    image_kind: "backdrop".to_string(),
                    relative_path: "works/distant-signal/backdrop-1280.webp".to_string(),
                    width: Some(1280),
                    height: Some(720),
                    fingerprint: "def456".to_string(),
                    dominant_color: None,
                }],
            )
            .await
            .expect("backdrop stored");

        let viewer = database
            .create_user("Viewer", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account created")
            .id;
        let state = AppState::new(melyxar_config::Config::default(), database, None, None);

        let detail = work_detail(&state, &crate::an_ordinary_account(viewer), episode.id)
            .await
            .expect("read")
            .expect("present");

        let up_series = detail
            .ancestry
            .iter()
            .find(|up| up.kind == WorkKind::Series)
            .expect("the series is among the ancestors");
        assert_eq!(up_series.logo.len(), 1, "the series carries its own mark");

        let up_season = detail
            .ancestry
            .iter()
            .find(|up| up.kind == WorkKind::Season)
            .expect("the season is among the ancestors");
        assert!(
            up_season.logo.is_empty(),
            "a season draws no mark of its own"
        );
        assert_eq!(
            up_series.logo.iter().map(|image| image.image_kind.as_str()).collect::<Vec<_>>(),
            vec!["logo"],
            "the mark of the series, and nothing else of its pictures"
        );
        assert_eq!(
            detail
                .images
                .iter()
                .filter(|image| image.image_kind == "backdrop")
                .map(|image| image.owner_id.as_str())
                .collect::<Vec<_>>(),
            vec![series.id.to_db_string().as_str()],
            "an episode with no backdrop of its own stands on its series'"
        );
    }
}
