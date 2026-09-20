//! Inventing a library large enough to measure against.
//!
//! Fifty films say nothing about a hundred thousand. Every fault that only
//! shows at scale (a page that asks one question per card, a menu that counts
//! the whole library to draw itself, an ordering that walks the table) looks
//! perfectly healthy on the collection this is developed against, and turns up
//! for the first time on somebody else's. So the collection is invented here,
//! in the shape a real one has, and the measuring is done against that.
//!
//! In the shape a real one has is the whole difficulty. A hundred thousand
//! rows in one table measure nothing: what a page costs is decided by what
//! hangs off each work and by how the titles spread out. So the titles begin
//! with every letter, the years cover sixty of them, the genres and the
//! studios are shared the way a catalogue shares them, a few thousand actors
//! are credited across the lot rather than one per film, and part of the
//! library is series with their seasons and episodes rather than films alone.
//!
//! Invented from nothing but a number, and the same every time: the same
//! request builds the same library, so two measurements a week apart are
//! measurements of the same thing.

use std::time::{Duration, Instant};

use melyxar_core::id::{LibraryId, LibraryRootId, MediaSourceId, UserId, WorkId};
use melyxar_core::work::WorkKind;
use melyxar_database::synthetic::{
    Invented, InventedCredit, InventedPicture, InventedProgress, InventedSource, InventedTrack,
    InventedWork, SharedNames, BENCH_LIBRARY,
};

use crate::{AppState, Result};

/// How many works met on their own there are for every work inside a series.
///
/// A collection is not made of series alone, and the budgets are written about
/// a grid: a library where nine works in ten are an episode is a library whose
/// grid holds a tenth of what was asked for, and the number that was measured
/// is then not the number that was promised. Five to one leaves both near
/// enough to the same figure that neither has to be explained.
const WORKS_PER_SERIES_WORK: i64 = 5;

/// How many works are written in one transaction.
///
/// Large enough that the cost of a transaction disappears against what it
/// carries, small enough that a run stopped halfway leaves a library which is
/// smaller than asked for rather than one that is half written.
const BATCH: usize = 500;

/// What filling left behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Filled {
    pub works: i64,
    pub files: i64,
    pub took: Duration,
}

/// What taking the invented library away took with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Emptied {
    pub works: i64,
    pub files: i64,
}

/// Fills an invented library of roughly this many works.
///
/// Roughly, because a series arrives whole: its seasons and its episodes are
/// written with it, and stopping in the middle of one would leave a series
/// with three of its five seasons. The count that comes back is the real one.
///
/// Refuses to touch a library that already exists. Emptying is a separate
/// request on purpose: it is the one that removes a hundred thousand works,
/// and it should never happen as a side effect of asking for a measurement.
pub async fn fill(
    state: &AppState,
    works: i64,
    told: impl Fn(i64, i64),
) -> Result<Filled> {
    let database = state.database();
    if database.bench_library().await?.is_some() {
        return Err(melyxar_core::Error::invalid_input(
            "an invented library is already here; empty it before filling another",
        )
        .into());
    }

    let viewer = viewer(state).await?;
    let started = Instant::now();
    let (library_id, root_id) = database.start_an_invented_library().await?;

    // A run that falls over halfway has to leave nothing behind. Without this,
    // the library it had already made is still there, the next request refuses
    // because of it, and somebody has to be told to empty a library they never
    // managed to fill.
    match write_them_all(state, library_id, root_id, viewer, works, told).await {
        Ok(()) => {}
        Err(error) => {
            empty(state).await?;
            return Err(error);
        }
    }

    let Invented { works, files } = database.settle_the_invented_library(library_id).await?;
    Ok(Filled {
        works,
        files,
        took: started.elapsed(),
    })
}

/// Invents and writes the works themselves, batch after batch.
async fn write_them_all(
    state: &AppState,
    library_id: LibraryId,
    root_id: LibraryRootId,
    viewer: UserId,
    works: i64,
    told: impl Fn(i64, i64),
) -> Result<()> {
    let database = state.database();
    let names = database
        .invent_the_shared_names(&words(&GENRES), &words(&STUDIOS), &people())
        .await?;

    let mut written = 0_i64;
    let mut in_series = 0_i64;
    let mut batch: Vec<InventedWork> = Vec::with_capacity(BATCH * 2);
    let mut index = 0_u64;

    while written < works {
        // A series arrives whole, sixty works at a time, so emitting one every
        // so many rounds would drown the library in episodes: at one round in
        // seven, nine works in ten are an episode and the grid holds a tenth
        // of what was asked for. Counted as it goes instead, so the share
        // lands where it is wanted whatever size a series turns out to be.
        let mut invented = match in_series * (1 + WORKS_PER_SERIES_WORK) <= written {
            true => a_series(index, &names),
            false => vec![a_film(index, &names)],
        };
        if invented.len() > 1 {
            in_series += invented.len() as i64;
        }
        written += invented.len() as i64;
        batch.append(&mut invented);
        index += 1;

        if batch.len() >= BATCH {
            database
                .invent_works(library_id, root_id, viewer, &batch)
                .await?;
            batch.clear();
            told(written, works);
        }
    }
    if !batch.is_empty() {
        database
            .invent_works(library_id, root_id, viewer, &batch)
            .await?;
    }
    told(written, works);
    Ok(())
}

/// Takes the invented library away, and nothing else.
///
/// Bounded by the name it was made under, so a real library is never within
/// reach of this. No file on the disk is touched, for the plain reason that
/// none was ever written.
pub async fn empty(state: &AppState) -> Result<Emptied> {
    let database = state.database();
    let Some(library) = database.bench_library().await? else {
        return Ok(Emptied { works: 0, files: 0 });
    };
    let removed = database.delete_library(library.id).await?;

    // A hundred thousand works leave half a gigabyte of room in the file,
    // which nothing here will ever write into again. Asked for rather than
    // insisted on: the library is already gone, and saying so failed because
    // somebody was reading a page at that moment would be a lie.
    if let Err(error) = database.reclaim_space().await {
        tracing::warn!(
            error = %error,
            "the room the invented library took could not be given back to the \
             disk; it stays in the database file, ready to be written into again"
        );
    }

    tracing::info!(
        works = removed.works,
        files = removed.files,
        "the invented library is gone"
    );
    Ok(Emptied {
        works: removed.works,
        files: removed.files,
    })
}

/// What an invented library holds right now, if one is here.
pub async fn what_is_there(state: &AppState) -> Result<Option<Invented>> {
    let Some(library) = state.database().bench_library().await? else {
        return Ok(None);
    };
    Ok(Some(
        state
            .database()
            .what_an_invented_library_holds(library.id)
            .await?,
    ))
}

/// Whose progress the invented library carries.
async fn viewer(state: &AppState) -> Result<UserId> {
    state
        .database()
        .user_by_name(crate::startup::DEFAULT_ACCOUNT_NAME)
        .await?
        .map(|(user, _)| user.id)
        .ok_or_else(|| melyxar_core::Error::invalid_input("this server has no account at all"))
        .map_err(Into::into)
}

/// One invented film, with everything a page reads about it.
fn a_film(index: u64, names: &SharedNames) -> InventedWork {
    let mut numbers = Numbers::seeded(index);
    let title = title_of(index);
    let year = numbers.between(1962, 2025) as i32;
    let runtime = numbers.between(72, 168) * 60_000;

    InventedWork {
        id: WorkId::new(),
        parent_id: None,
        ordinal: None,
        kind: WorkKind::Movie,
        sort_title: melyxar_library::naming::sort_title(&title),
        title: title.clone(),
        release_year: Some(year),
        runtime_ms: Some(runtime),
        community_rating: Some(rating(&mut numbers)),
        age_rating_label: age_rating(&mut numbers),
        dominant_color: colour(&mut numbers),
        child_count: 0,
        added_seconds_ago: added_seconds_ago(index),
        tagline: tagline(&mut numbers),
        overview: overview(&title, year, &mut numbers),
        external_id: format!("bench-{index}"),
        genres: chosen(&mut numbers, &names.genres, 1, 3),
        studios: chosen(&mut numbers, &names.studios, 1, 1),
        credits: cast(&mut numbers, names),
        pictures: every_picture(),
        sources: vec![a_file(&title, year, runtime, &mut numbers)],
        watched: how_far(&mut numbers, runtime),
        unwatched: None,
    }
}

/// One invented series, with its seasons and their episodes.
///
/// The series first, then each season followed by its own episodes. The order
/// is not a matter of taste: a batch is written parent by parent, and a work
/// pointing at a parent that is not there yet is a work the database refuses.
fn a_series(index: u64, names: &SharedNames) -> Vec<InventedWork> {
    let mut numbers = Numbers::seeded(index);
    let title = title_of(index);
    let year = numbers.between(1998, 2024) as i32;
    let seasons = numbers.between(2, 6);

    let series_id = WorkId::new();
    let mut beneath = Vec::new();
    let mut unwatched_of_series = 0;

    for season_number in 1..=seasons {
        let season_id = WorkId::new();
        let episodes = numbers.between(6, 22);
        let mut unwatched_of_season = 0;
        let mut of_this_season = Vec::new();

        for episode_number in 1..=episodes {
            let runtime = numbers.between(21, 54) * 60_000;
            let name = format!("{title} S{season_number:02}E{episode_number:02}");
            let watched = how_far(&mut numbers, runtime);
            if !matches!(watched, Some(InventedProgress { state: "watched", .. })) {
                unwatched_of_season += 1;
            }
            of_this_season.push(InventedWork {
                id: WorkId::new(),
                parent_id: Some(season_id),
                ordinal: Some(episode_number as i32),
                kind: WorkKind::Episode,
                title: episode_title(&mut numbers),
                sort_title: melyxar_library::naming::sort_title(&name),
                release_year: Some(year + season_number as i32 - 1),
                runtime_ms: Some(runtime),
                community_rating: Some(rating(&mut numbers)),
                age_rating_label: None,
                dominant_color: colour(&mut numbers),
                child_count: 0,
                added_seconds_ago: added_seconds_ago(index) + episode_number,
                tagline: String::new(),
                overview: overview(&name, year, &mut numbers),
                external_id: format!("bench-{index}-{season_number}-{episode_number}"),
                genres: Vec::new(),
                studios: Vec::new(),
                credits: Vec::new(),
                pictures: picture_sizes("poster"),
                sources: vec![a_file(&name, year, runtime, &mut numbers)],
                watched,
                unwatched: None,
            });
        }

        unwatched_of_series += unwatched_of_season;
        beneath.push(InventedWork {
            id: season_id,
            parent_id: Some(series_id),
            ordinal: Some(season_number as i32),
            kind: WorkKind::Season,
            title: format!("Season {season_number}"),
            sort_title: melyxar_library::naming::sort_title(&format!(
                "{title} {season_number:02}"
            )),
            release_year: Some(year + season_number as i32 - 1),
            runtime_ms: None,
            community_rating: None,
            age_rating_label: None,
            dominant_color: colour(&mut numbers),
            child_count: episodes,
            added_seconds_ago: added_seconds_ago(index),
            tagline: String::new(),
            overview: String::new(),
            external_id: format!("bench-{index}-{season_number}"),
            genres: Vec::new(),
            studios: Vec::new(),
            credits: Vec::new(),
            pictures: picture_sizes("poster"),
            sources: Vec::new(),
            watched: None,
            unwatched: Some(unwatched_of_season),
        });
        beneath.append(&mut of_this_season);
    }

    let mut written = vec![InventedWork {
        id: series_id,
        parent_id: None,
        ordinal: None,
        kind: WorkKind::Series,
        sort_title: melyxar_library::naming::sort_title(&title),
        title: title.clone(),
        release_year: Some(year),
        runtime_ms: None,
        community_rating: Some(rating(&mut numbers)),
        age_rating_label: age_rating(&mut numbers),
        dominant_color: colour(&mut numbers),
        child_count: seasons,
        added_seconds_ago: added_seconds_ago(index),
        tagline: tagline(&mut numbers),
        overview: overview(&title, year, &mut numbers),
        external_id: format!("bench-{index}"),
        genres: chosen(&mut numbers, &names.genres, 1, 3),
        studios: chosen(&mut numbers, &names.studios, 1, 1),
        credits: cast(&mut numbers, names),
        pictures: every_picture(),
        sources: Vec::new(),
        watched: None,
        unwatched: Some(unwatched_of_series),
    }];
    written.append(&mut beneath);
    written
}

/// One invented file, described the way the analyser describes a real one.
fn a_file(name: &str, year: i32, runtime_ms: i64, numbers: &mut Numbers) -> InventedSource {
    let height = *numbers.pick(&[720, 1080, 1080, 1080, 2160]);
    let bitrate = numbers.between(2_000_000, 18_000_000);

    InventedSource {
        id: MediaSourceId::new(),
        relative_path: format!("{name} ({year}).mkv"),
        container: "matroska".to_string(),
        duration_ms: runtime_ms,
        overall_bitrate: bitrate,
        size_bytes: bitrate / 8 * runtime_ms / 1_000,
        tracks: vec![
            InventedTrack {
                stream_index: 0,
                kind: "video",
                language: None,
                title: None,
                is_default: true,
                codec: numbers.pick(&["h264", "hevc", "av1"]).to_string(),
                profile: Some("main".to_string()),
                bitrate: Some(bitrate),
                width: Some(height * 16 / 9),
                height: Some(height),
                frame_rate: Some(*numbers.pick(&[23.976, 24.0, 25.0])),
                pixel_format: Some("yuv420p".to_string()),
                bit_depth: Some(8),
                channels: None,
                channel_layout: None,
                sample_rate: None,
                subtitle_layout: None,
            },
            audio_track(1, "fra", true, numbers),
            audio_track(2, "eng", false, numbers),
            subtitle_track(3, "fra"),
            subtitle_track(4, "eng"),
        ],
    }
}

fn audio_track(
    stream_index: i32,
    language: &str,
    is_default: bool,
    numbers: &mut Numbers,
) -> InventedTrack {
    let channels = *numbers.pick(&[2, 6, 6, 8]);
    InventedTrack {
        stream_index,
        kind: "audio",
        language: Some(language.to_string()),
        title: None,
        is_default,
        codec: numbers.pick(&["aac", "eac3", "dts"]).to_string(),
        profile: None,
        bitrate: Some(i64::from(channels) * 128_000),
        width: None,
        height: None,
        frame_rate: None,
        pixel_format: None,
        bit_depth: None,
        channels: Some(channels),
        channel_layout: Some(match channels {
            2 => "stereo".to_string(),
            6 => "5.1".to_string(),
            _ => "7.1".to_string(),
        }),
        sample_rate: Some(48_000),
        subtitle_layout: None,
    }
}

fn subtitle_track(stream_index: i32, language: &str) -> InventedTrack {
    InventedTrack {
        stream_index,
        kind: "subtitle",
        language: Some(language.to_string()),
        title: None,
        is_default: false,
        codec: "subrip".to_string(),
        profile: None,
        bitrate: None,
        width: None,
        height: None,
        frame_rate: None,
        pixel_format: None,
        bit_depth: None,
        channels: None,
        channel_layout: None,
        sample_rate: None,
        subtitle_layout: Some("text".to_string()),
    }
}

/// Every size of every picture a film carries.
fn every_picture() -> Vec<InventedPicture> {
    [
        picture_sizes("poster"),
        picture_sizes("backdrop"),
        picture_sizes("logo"),
    ]
    .concat()
}

/// The sizes one kind of picture is stored in, as the server stores them.
fn picture_sizes(kind: &'static str) -> Vec<InventedPicture> {
    let widths: &[u32] = match kind {
        "poster" => &melyxar_ffmpeg::images::POSTER_WIDTHS,
        "backdrop" => &melyxar_ffmpeg::images::BACKDROP_WIDTHS,
        _ => &melyxar_ffmpeg::images::LOGO_WIDTHS,
    };
    let shape = match kind {
        "poster" => (2, 3),
        "backdrop" => (16, 9),
        _ => (5, 2),
    };
    widths
        .iter()
        .map(|width| InventedPicture {
            kind,
            width: *width as i32,
            height: (*width as i32) * shape.1 / shape.0,
        })
        .collect()
}

/// The cast and crew of one invented work.
///
/// A director, a writer, and the names a page shows faces for. Drawn from the
/// shared pool rather than invented per film, because that is how a catalogue
/// works and because the filmography of a person is a page of its own.
fn cast(numbers: &mut Numbers, names: &SharedNames) -> Vec<InventedCredit> {
    if names.people.is_empty() {
        return Vec::new();
    }
    let mut credits = Vec::with_capacity(14);
    for (ordinal, role) in ["director", "writer"].iter().enumerate() {
        credits.push(InventedCredit {
            person_id: *numbers.pick(&names.people),
            role: (*role).to_string(),
            character_name: None,
            ordinal: ordinal as i32,
        });
    }
    for ordinal in 0..numbers.between(6, 12) {
        credits.push(InventedCredit {
            person_id: *numbers.pick(&names.people),
            role: "actor".to_string(),
            character_name: Some(numbers.pick(&CHARACTERS).to_string()),
            ordinal: ordinal as i32,
        });
    }
    credits
}

/// Where somebody left a work, for the share of them somebody has opened.
///
/// A home page is made of what is half watched, so a library where nothing
/// ever was measures a page nobody has.
fn how_far(numbers: &mut Numbers, runtime_ms: i64) -> Option<InventedProgress> {
    // Spread over the last two years, and never two at the same instant: the
    // row of what to carry on with is ordered by when each was last played,
    // and works sharing one instant make the server read every one of them
    // before it can show twenty.
    let seconds_since_played = numbers.between(60, 2 * 365 * 24 * 60 * 60);
    let drawn = numbers.below(IN_A_THOUSAND);
    if drawn < LEFT_HALFWAY {
        return Some(InventedProgress {
            position_ms: runtime_ms * numbers.between(10, 80) / 100,
            state: "in_progress",
            seconds_since_played,
        });
    }
    if drawn < LEFT_HALFWAY + WATCHED_THROUGH {
        return Some(InventedProgress {
            position_ms: runtime_ms,
            state: "watched",
            seconds_since_played,
        });
    }
    None
}

/// What the shares below are drawn out of.
const IN_A_THOUSAND: u64 = 1_000;

/// How many works in a thousand somebody has left halfway.
///
/// Deliberately small, because a person is. Left at a share of the collection
/// it would mean thousands of unfinished films on a large one, which is not a
/// collection anybody has: it would measure a row of what to carry on with
/// against a number no real library reaches, and call the result reactivity.
const LEFT_HALFWAY: u64 = 1;

/// How many works in a thousand somebody has watched right through.
///
/// A share of the collection, this one, because that is what watching a
/// collection over years looks like.
const WATCHED_THROUGH: u64 = 200;

/// How long ago a work was added, in seconds.
///
/// A minute apart, oldest first, so a collection built in one run still reads
/// as one built over years: an ordering by date has something to order, and
/// the newest of a hundred thousand is one work rather than all of them.
fn added_seconds_ago(index: u64) -> i64 {
    (index as i64).saturating_mul(60)
}

fn rating(numbers: &mut Numbers) -> f64 {
    (numbers.between(41, 95) as f64) / 10.0
}

fn age_rating(numbers: &mut Numbers) -> Option<String> {
    match numbers.below(5) {
        0 => None,
        other => Some(["Tous publics", "-10", "-12", "-16"][other as usize - 1].to_string()),
    }
}

fn colour(numbers: &mut Numbers) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        numbers.between(24, 200),
        numbers.between(24, 200),
        numbers.between(24, 200)
    )
}

fn tagline(numbers: &mut Numbers) -> String {
    numbers.pick(&TAGLINES).to_string()
}

fn overview(title: &str, year: i32, numbers: &mut Numbers) -> String {
    format!(
        "{title} ({year}). {} {}",
        numbers.pick(&TAGLINES),
        numbers.pick(&TAGLINES)
    )
}

fn episode_title(numbers: &mut Numbers) -> String {
    format!(
        "{} {}",
        numbers.pick(&FIRST_WORDS),
        numbers.pick(&SECOND_WORDS)
    )
}

/// A few of a pool, drawn without drawing the same one twice.
fn chosen<T: Copy + PartialEq>(
    numbers: &mut Numbers,
    from: &[T],
    least: i64,
    most: i64,
) -> Vec<T> {
    if from.is_empty() {
        return Vec::new();
    }
    let wanted = numbers.between(least, most) as usize;
    let mut drawn = Vec::with_capacity(wanted);
    // Bounded by the pool rather than by the draw: asking for three out of two
    // would otherwise turn on a run of numbers that never repeats.
    for _ in 0..wanted.min(from.len()) {
        for _ in 0..from.len() {
            let one = *numbers.pick(from);
            if !drawn.contains(&one) {
                drawn.push(one);
                break;
            }
        }
    }
    drawn
}

/// The title of the work at this rank, spread over every letter.
///
/// Built from the rank rather than drawn, so the titles cover the alphabet
/// evenly instead of clustering: a grid asked to jump to a letter, and a
/// search on a prefix, are both measured by how many rows the letter covers.
fn title_of(index: u64) -> String {
    let first = FIRST_WORDS[(index % FIRST_WORDS.len() as u64) as usize];
    let second = SECOND_WORDS[((index / FIRST_WORDS.len() as u64) % SECOND_WORDS.len() as u64)
        as usize];
    let round = index / (FIRST_WORDS.len() as u64 * SECOND_WORDS.len() as u64);
    match round {
        0 => format!("{first} {second}"),
        // A collection large enough to run out of names has sequels in it,
        // which is also what puts two films of nearly the same name next to
        // each other in an ordering.
        other => format!("{first} {second} {}", other + 1),
    }
}

/// The words a name is built from, one list per slot.
///
/// Invented, and deliberately spread over the alphabet: a library whose titles
/// all begin with the same handful of letters measures a grid nobody has.
const FIRST_WORDS: [&str; 34] = [
    "Ardent", "Blue", "Crooked", "Distant", "Eastern", "Faded", "Golden", "Hidden", "Idle",
    "Jagged", "Keen", "Lonely", "Marble", "Northern", "Open", "Pale", "Quiet", "Restless",
    "Silver", "Tender", "Upper", "Velvet", "Winter", "Yellow", "Zealous", "Amber", "Brittle",
    "Crimson", "Dusty", "Ember", "Frozen", "Grave", "Hollow", "Iron",
];

const SECOND_WORDS: [&str; 30] = [
    "Harbour", "Orchard", "Signal", "Lantern", "Meridian", "Passage", "Quarry", "Ribbon",
    "Station", "Tide", "Vault", "Window", "Anchor", "Bridge", "Chapel", "Dune", "Echo", "Forest",
    "Garden", "Hour", "Island", "Junction", "Kiln", "Ledger", "Mirror", "Needle", "Oath",
    "Pier", "Road", "Summit",
];

/// The names a cast is drawn from, built the same way so nobody has to write
/// out several thousand of them.
const GIVEN_NAMES: [&str; 24] = [
    "Adele", "Bruno", "Celia", "Damien", "Elsa", "Fabien", "Greta", "Hugo", "Ines", "Jonas",
    "Klara", "Luc", "Mira", "Noah", "Olga", "Pierre", "Quentin", "Rosa", "Samir", "Tessa",
    "Ulysse", "Vera", "Willem", "Yara",
];

const FAMILY_NAMES: [&str; 26] = [
    "Alvarez", "Baumann", "Costa", "Delacroix", "Engel", "Ferreira", "Gauthier", "Hartmann",
    "Iversen", "Jansen", "Kowalski", "Lindqvist", "Moreau", "Novak", "Olsen", "Petit", "Quintana",
    "Rinaldi", "Sorensen", "Tremblay", "Ueda", "Vasquez", "Weber", "Xiang", "Yilmaz", "Zanetti",
];

const CHARACTERS: [&str; 12] = [
    "The Captain",
    "The Archivist",
    "The Neighbour",
    "The Surveyor",
    "The Youngest",
    "The Stranger",
    "The Doctor",
    "The Pilot",
    "The Teacher",
    "The Sister",
    "The Watchman",
    "The Guest",
];

const GENRES: [&str; 18] = [
    "Action",
    "Animation",
    "Aventure",
    "Comédie",
    "Documentaire",
    "Drame",
    "Fantastique",
    "Guerre",
    "Histoire",
    "Horreur",
    "Musique",
    "Mystère",
    "Policier",
    "Romance",
    "Science-fiction",
    "Thriller",
    "Western",
    "Familial",
];

const STUDIOS: [&str; 16] = [
    "Atelier Nord",
    "Bureau des Images",
    "Cinéma Meridien",
    "Delta Pictures",
    "Estuaire Films",
    "Fontaine Studio",
    "Grand Quai",
    "Horizon Bleu",
    "Ithaque",
    "Jour Blanc",
    "Kestrel House",
    "Lumière Basse",
    "Maison Verte",
    "Nord Ouest",
    "Ombre Portée",
    "Pavillon Sud",
];

const TAGLINES: [&str; 8] = [
    "Ce qui a été laissé derrière finit toujours par revenir.",
    "Une nuit suffit à défaire ce que dix ans ont construit.",
    "Personne ne quitte la vallée sans y avoir laissé quelque chose.",
    "Le silence coûte plus cher que la vérité.",
    "Tout le monde savait, et personne n'a rien dit.",
    "On ne rentre jamais deux fois dans la même maison.",
    "Ils avaient un été pour tout changer.",
    "La ville dort, mais pas ses habitants.",
];

/// Names the shared pool is filled with.
fn people() -> Vec<String> {
    let mut names = Vec::with_capacity(GIVEN_NAMES.len() * FAMILY_NAMES.len());
    for given in GIVEN_NAMES {
        for family in FAMILY_NAMES {
            names.push(format!("{given} {family}"));
        }
    }
    names
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| (*word).to_string()).collect()
}

/// A run of numbers that looks random and is the same every time.
///
/// Written here rather than drawn from a library: a bench whose library is
/// different on every run is a bench whose two measurements cannot be
/// compared, and that is the only property wanted of these numbers.
struct Numbers(u64);

impl Numbers {
    fn seeded(from: u64) -> Self {
        Self(from.wrapping_mul(0x9e37_79b9_7f4a_7c15).wrapping_add(1))
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound.max(1)
    }

    /// Somewhere between the two, both included.
    fn between(&mut self, least: i64, most: i64) -> i64 {
        if most <= least {
            return least;
        }
        least + self.below((most - least + 1) as u64) as i64
    }

    fn pick<'a, T>(&mut self, from: &'a [T]) -> &'a T {
        &from[self.below(from.len() as u64) as usize]
    }
}

/// The name the invented library goes under, for whoever has to say it out
/// loud.
pub const LIBRARY_NAME: &str = BENCH_LIBRARY;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_request_builds_the_same_library() {
        let names = SharedNames::default();
        let first = a_film(42, &names);
        let second = a_film(42, &names);
        assert_eq!(first.title, second.title);
        assert_eq!(first.release_year, second.release_year);
        assert_eq!(first.community_rating, second.community_rating);
        assert_eq!(first.sources[0].size_bytes, second.sources[0].size_bytes);
    }

    #[test]
    fn the_titles_cover_the_alphabet_rather_than_a_corner_of_it() {
        let letters: std::collections::BTreeSet<char> = (0..2_000)
            .map(title_of)
            .filter_map(|title| title.chars().next())
            .collect();
        assert!(
            letters.len() >= 20,
            "a library whose titles all start with the same letters measures a \
             grid nobody has: {letters:?}"
        );
    }

    #[test]
    fn a_collection_larger_than_the_names_keeps_going() {
        let combinations = (FIRST_WORDS.len() * SECOND_WORDS.len()) as u64;
        assert_ne!(title_of(0), title_of(combinations));
        assert_ne!(title_of(combinations), title_of(combinations * 2));
    }

    #[test]
    fn a_series_arrives_whole() {
        let names = SharedNames::default();
        let written = a_series(6, &names);
        let series: Vec<_> = written
            .iter()
            .filter(|work| work.kind == WorkKind::Series)
            .collect();
        assert_eq!(series.len(), 1, "exactly one series");

        let seasons: Vec<_> = written
            .iter()
            .filter(|work| work.kind == WorkKind::Season)
            .collect();
        assert_eq!(
            seasons.len() as i64,
            series[0].child_count,
            "a series counts the seasons that came with it"
        );
        for season in &seasons {
            assert_eq!(season.parent_id, Some(series[0].id));
            let episodes = written
                .iter()
                .filter(|work| work.parent_id == Some(season.id))
                .count();
            assert_eq!(episodes as i64, season.child_count);
        }
        // Nothing ever points at a parent that is not written yet: the
        // database refuses a child whose parent is still to come.
        let mut already = std::collections::BTreeSet::new();
        for work in &written {
            if let Some(parent) = work.parent_id {
                assert!(
                    already.contains(&parent),
                    "{} is written before what holds it",
                    work.title
                );
            }
            already.insert(work.id);
        }
    }

    #[test]
    fn a_draw_never_takes_the_same_name_twice() {
        let pool: Vec<u8> = (0..4).collect();
        let mut numbers = Numbers::seeded(1);
        for _ in 0..50 {
            let drawn = chosen(&mut numbers, &pool, 1, 3);
            let unique: std::collections::BTreeSet<u8> = drawn.iter().copied().collect();
            assert_eq!(drawn.len(), unique.len(), "{drawn:?}");
        }
        assert!(chosen(&mut numbers, &[] as &[u8], 1, 3).is_empty());
        // Asking for more than the pool holds gives the pool, not a loop.
        assert_eq!(chosen(&mut numbers, &pool, 9, 9).len(), pool.len());
    }

    #[test]
    fn every_picture_comes_in_the_sizes_the_server_serves() {
        let pictures = every_picture();
        let posters: Vec<_> = pictures
            .iter()
            .filter(|picture| picture.kind == "poster")
            .collect();
        assert_eq!(posters.len(), melyxar_ffmpeg::images::POSTER_WIDTHS.len());
        assert!(pictures.iter().all(|picture| picture.height > 0));
    }

    #[test]
    fn a_number_stays_between_the_two_it_was_given() {
        let mut numbers = Numbers::seeded(7);
        for _ in 0..500 {
            let drawn = numbers.between(3, 9);
            assert!((3..=9).contains(&drawn), "{drawn}");
        }
        assert_eq!(numbers.between(5, 5), 5);
        assert_eq!(numbers.between(5, 2), 5);
    }
}
