//! Browsing: libraries, grids and detail pages.
//!
//! The shapes sent here are written for the screens that read them, never
//! taken from the database as they are. A grid is given exactly what a card
//! draws and nothing else, because a page of sixty cards carrying a synopsis
//! each is ten times the weight for text nobody reads there.

use crate::account::Viewer;
use crate::identifiers::{parse_library, parse_person, parse_work};
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::Request;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use melyxar_app::browse::{BrowseRequest, Initial, WorkCard, WorkOrder, DEFAULT_PAGE};
use melyxar_app::detail::{CarryOn, Credit, Version, WorkDetail};
use melyxar_app::picture::StoredImage;
use melyxar_app::AppState;
use melyxar_core::media::TrackKind;
use melyxar_core::time::Millis;
use melyxar_core::library::LibraryKind;
use melyxar_core::work::{IdentificationNote, PlaybackState};
use serde::{Deserialize, Serialize};

use crate::error::{Result, ServerError};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/libraries", axum::routing::get(libraries))
        .route(
            "/api/v1/libraries/{id}/filters",
            axum::routing::get(filters),
        )
        .route("/api/v1/works", axum::routing::get(works))
        .route("/api/v1/works/{id}", axum::routing::get(work))
        .route(
            "/api/v1/works/{id}/favourite",
            axum::routing::put(set_favourite),
        )
        .route(
            "/api/v1/works/{id}/watched",
            axum::routing::put(set_watched),
        )
        .route("/api/v1/works/{id}/pinned", axum::routing::put(set_pinned))
        .route(
            "/api/v1/works/{id}/trailers/{rank}",
            axum::routing::get(trailer),
        )
        .route("/api/v1/works/{id}/photo", axum::routing::get(photo))
        .route("/api/v1/people/{id}", axum::routing::get(person))
        .route("/api/v1/home", axum::routing::get(home))
}

/// Hands over a trailer sitting next to the film.
///
/// Named by its place in the list the detail page was given, so nothing a
/// client sends is ever treated as a path. A trailer hosted elsewhere never
/// comes through here: it is watched where it lives, and this server does not
/// go and fetch someone else's video.
async fn trailer(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path((id, rank)): Path<(String, usize)>,
    request: Request<Body>,
) -> Response {
    match serve_trailer(&state, &who, &id, rank, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn serve_trailer(
    state: &AppState,
    who: &melyxar_core::user::User,
    id: &str,
    rank: usize,
    request: Request<Body>,
) -> Result<Response> {
    let work_id = parse_work(id)?;
    let detail = melyxar_app::detail::work_detail(state, who, work_id)
        .await?
        .ok_or_else(|| ServerError::not_found("no work with that identifier"))?;

    let file = detail
        .trailers
        .get(rank)
        .and_then(|trailer| trailer.local.as_ref())
        .ok_or_else(|| ServerError::not_found("no trailer of this work sits on the disk"))?;

    crate::serve_the_file(&file.path, request).await
}

/// Hands over a photo as it is on the disk, for looking at it whole.
///
/// Served untouched and read by the browser as it is: a photo is seen one at
/// a time, and preparing every photo at full size would take as much room
/// again as the photos themselves. Its cards are what is prepared ahead.
async fn photo(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    request: Request<Body>,
) -> Response {
    match serve_photo(&state, &who, &id, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn serve_photo(
    state: &AppState,
    who: &melyxar_core::user::User,
    id: &str,
    request: Request<Body>,
) -> Result<Response> {
    let work_id = parse_work(id)?;
    let detail = melyxar_app::detail::work_detail(state, who, work_id)
        .await?
        .filter(|detail| detail.work.kind == melyxar_core::work::WorkKind::Photo)
        .ok_or_else(|| ServerError::not_found("no photo with that identifier"))?;
    let file = detail
        .versions
        .iter()
        .find(|version| version.missing_since.is_none())
        .ok_or_else(|| ServerError::not_found("the photo is not on the disk at the moment"))?;
    crate::serve_the_file(std::path::Path::new(&file.path), request).await
}

// ---------------------------------------------------------------------------
// Libraries
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct LibraryView {
    id: String,
    name: String,
    kind: &'static str,
    /// What the grid of this library holds, so a menu can show a count without
    /// asking for a page first.
    works: i64,
    /// Bumped whenever anything in the library moves. A client keeps it and
    /// asks whether it changed instead of fetching everything again.
    version: i64,
    /// Whether a scan of this library reads every film for where its picture
    /// can be started, rather than leaving it to the upkeep.
    key_frames_during_scan: bool,
    /// The same for the thumbnails of the playback bar.
    thumbnails_during_scan: bool,
    /// The language its films are described in, as a two letter code.
    metadata_language: String,
    roots: Vec<RootView>,
    /// How many of its files were taken out of it and left on the disk.
    set_aside: i64,
}

#[derive(Debug, Serialize)]
struct RootView {
    /// What renaming it needs, since a label is what every log line shows
    /// instead of the path and one that reads badly is worth putting right.
    id: String,
    label: String,
    /// Its whole path on the server's disk. Shown only on the screen where
    /// roots are managed, for the administrator who has to tell two roots
    /// apart by more than a label they gave one themselves.
    path: String,
    /// What the server may actually do with this folder, told by trying.
    access: &'static str,
    /// Stable code the interface turns into a sentence in its own language.
    explanation_code: &'static str,
}

async fn libraries(
    State(state): State<AppState>,
    Viewer(who): Viewer,
) -> Result<Json<Vec<LibraryView>>> {
    let summaries = melyxar_app::catalogue::libraries(&state, &who).await?;
    Ok(Json(
        summaries
            .into_iter()
            .map(|library| LibraryView {
                id: library.id.to_string(),
                name: library.name,
                kind: library.kind.as_str(),
                works: library.works,
                version: library.version,
                key_frames_during_scan: library.options.key_frames_during_scan,
                thumbnails_during_scan: library.options.thumbnails_during_scan,
                metadata_language: library.metadata_language,
                set_aside: library.set_aside,
                roots: library
                    .roots
                    .into_iter()
                    .map(|root| RootView {
                        id: root.id.to_string(),
                        label: root.label,
                        path: root.path.to_string_lossy().into_owned(),
                        access: root.access.as_str(),
                        explanation_code: root.access.explanation_code(),
                    })
                    .collect(),
            })
            .collect(),
    ))
}

#[derive(Debug, Serialize)]
struct FiltersView {
    /// Only what is actually in the library: offering a genre nobody has leads
    /// to an empty grid and looks like a fault.
    genres: Vec<CountedView>,
    decades: Vec<CountedDecade>,
    /// The letters titles really start with, in order, the bucket for
    /// everything else first.
    initials: Vec<CountedView>,
}

#[derive(Debug, Serialize)]
struct CountedView {
    name: String,
    works: i64,
}

#[derive(Debug, Serialize)]
struct CountedDecade {
    decade: i32,
    works: i64,
}

async fn filters(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<FiltersView>> {
    let found =
        melyxar_app::catalogue::filters(&state, Some(parse_library(&id)?), &who).await?;
    Ok(Json(FiltersView {
        genres: found
            .genres
            .into_iter()
            .map(|(name, works)| CountedView { name, works })
            .collect(),
        decades: found
            .decades
            .into_iter()
            .map(|(decade, works)| CountedDecade { decade, works })
            .collect(),
        initials: found
            .initials
            .into_iter()
            .map(|(name, works)| CountedView { name, works })
            .collect(),
    }))
}

// ---------------------------------------------------------------------------
// Grids
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct BrowseParams {
    library: Option<String>,
    order: Option<WorkOrder>,
    #[serde(default)]
    descending: bool,
    /// The last card of the page before. A position rather than a number, so
    /// a film added while someone reads never doubles or hides a card.
    after: Option<String>,
    limit: Option<i64>,
    genre: Option<String>,
    decade: Option<i32>,
    search: Option<String>,
    #[serde(default)]
    unidentified: bool,
    /// Only what this account marked. A view rather than a library, which is
    /// why it is a narrowing of the grid and not an address of its own: it
    /// sorts, filters and pages exactly like everything else.
    #[serde(default)]
    favourites: bool,
    /// Only the libraries of one kind, whichever libraries those are. What
    /// the search's scope filter narrows by, and what a row of one kind is
    /// read from: a collection of films spread over four disks is four
    /// libraries and one category.
    kind: Option<LibraryKind>,
    /// One letter, or the sign standing for everything that starts with none.
    initial: Option<String>,
}

#[derive(Debug, Serialize)]
struct PageView {
    cards: Vec<CardView>,
    /// What to pass as `after` for the next page. Absent on the last one.
    next: Option<String>,
}

#[derive(Debug, Serialize)]
struct CardView {
    id: String,
    title: String,
    year: Option<i32>,
    runtime_minutes: Option<i64>,
    rating: Option<f64>,
    /// pending, identified, unidentified or manual. A card shows a marker for
    /// anything but identified, so a film nobody recognised stays visible
    /// instead of being quietly set aside.
    identification: &'static str,
    /// Why the last look up did not name it, when one has run and failed.
    /// Absent for a film nobody has looked up yet, which is itself the answer.
    identification_note: Option<&'static str>,
    /// Drawn under the picture while it loads, so a grid has colour from the
    /// first moment rather than grey holes.
    color: Option<String>,
    poster: Vec<ImageView>,
    /// A picture wider than it is tall, for the rows that lie a card down.
    /// Empty everywhere else, and empty where there is nothing wide to show,
    /// which such a row answers by falling back to the poster.
    wide: Vec<ImageView>,
    /// movie, series, season or episode. What a card offers depends on it: a
    /// film plays, a series is opened.
    kind: &'static str,
    /// Which library it came from, so a card can say where it lives without
    /// the page asking a second time.
    library: String,
    /// not_started, in_progress or watched, for the account asking.
    seen: &'static str,
    /// Where they stopped, in seconds, only where they stopped partway.
    resume_from_seconds: Option<i64>,
    favourite: bool,
    /// Episodes below, and how many of those are left to watch. Both nothing
    /// for a film, which holds none.
    episodes: i64,
    unwatched: i64,
    /// The copy a play button on the card would start, when one is on disk.
    source: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
struct ImageView {
    url: String,
    width: Option<i32>,
    height: Option<i32>,
}

async fn works(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Query(params): Query<BrowseParams>,
) -> Result<Json<PageView>> {
    let request = BrowseRequest {
        library_id: params.library.as_deref().map(parse_library).transpose()?,
        // What this account may read, and whose marks the cards come back
        // wearing, are not the client's to send: both are put on by the use
        // case, which is the one place that knows them.
        within: None,
        viewer: None,
        order: params.order.unwrap_or(WorkOrder::Title),
        descending: params.descending,
        after: params.after.as_deref().map(parse_work).transpose()?,
        limit: params.limit.unwrap_or(DEFAULT_PAGE),
        genre: params.genre,
        decade: params.decade,
        search: params.search.filter(|value| !value.trim().is_empty()),
        unidentified_only: params.unidentified,
        identified_only: false,
        favourites_only: params.favourites,
        library_kind: params.kind,
        catalogued_only: false,
        // A letter nobody could mean is refused rather than quietly ignored:
        // a grid that answers everything to a narrowing looks broken.
        initial: params
            .initial
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(|value| {
                Initial::parse(value)
                    .ok_or_else(|| ServerError::invalid_input("that is not a letter"))
            })
            .transpose()?,
    };

    let page = melyxar_app::catalogue::browse(&state, &request, &who).await?;
    Ok(Json(PageView {
        cards: page.cards.iter().map(card_view).collect(),
        next: page.next.map(|id| id.to_string()),
    }))
}

fn card_view(card: &WorkCard) -> CardView {
    CardView {
        id: card.id.to_string(),
        title: card.title.clone(),
        year: card.release_year,
        runtime_minutes: card.runtime.map(whole_minutes),
        rating: card.community_rating,
        identification: card.identification.as_str(),
        identification_note: card.identification_note.map(IdentificationNote::as_str),
        color: card.dominant_color.clone(),
        poster: card.poster.iter().map(image_view).collect(),
        wide: card.wide.iter().map(image_view).collect(),
        kind: card.kind.as_str(),
        library: card.library_id.to_string(),
        // A read nobody was named for carries nobody's marks, and answers as
        // a work nobody has touched rather than by leaving the fields out:
        // one shape of card is easier to draw than two.
        seen: card
            .state
            .as_ref()
            .map_or(PlaybackState::NotStarted, |state| state.seen)
            .as_str(),
        resume_from_seconds: card
            .state
            .as_ref()
            .and_then(|state| state.resume_from)
            .map(|position| position.get() / 1_000),
        favourite: card.state.as_ref().is_some_and(|state| state.favourite),
        episodes: card.state.as_ref().map_or(0, |state| state.episodes),
        unwatched: card.state.as_ref().map_or(0, |state| state.unwatched),
        source: card
            .state
            .as_ref()
            .and_then(|state| state.source_id)
            .map(|id| id.to_string()),
    }
}

/// The pictures of one kind a film holds, addressed as the interface asks for
/// them.
///
/// A film carries every kind of picture in one list, and each field of the
/// page takes the kind it shows: a title image among the posters would be a
/// card with a wordmark on it.
/// The name of a work, when it has one of its own rather than its number
/// written out in English.
fn its_own_name(
    kind: melyxar_core::work::WorkKind,
    ordinal: Option<i32>,
    title: &str,
) -> Option<String> {
    (!melyxar_app::episodes::is_only_a_number(kind, ordinal, title)).then(|| title.to_string())
}

fn child_view(child: &melyxar_app::detail::Child) -> ChildView {
    ChildView {
        id: child.work.id.to_string(),
        kind: child.work.kind.as_str(),
        number: child.work.ordinal,
        title: its_own_name(child.work.kind, child.work.ordinal, &child.work.title),
        runtime_minutes: child.work.runtime.map(whole_minutes),
        child_count: child.work.child_count,
        unwatched: child.work.unwatched,
        watched: child.work.watched,
        resume_from_seconds: child.work.resume_from.map(Millis::as_seconds_f64),
        playable: child.work.playable,
        identification: child.work.identification.as_str(),
        color: child.work.dominant_color.clone(),
        poster: pictures_of(&child.poster, "poster"),
        source_id: child.work.source_id.map(|id| id.to_string()),
    }
}

fn next_episode_view(carry: &CarryOn) -> NextEpisodeView {
    NextEpisodeView {
        id: carry.id.to_string(),
        season: carry.season,
        episode: carry.episode,
        title: carry.has_own_name.then(|| carry.title.clone()),
        source_id: carry.source_id.map(|id| id.to_string()),
    }
}

fn pictures_of(images: &[StoredImage], kind: &str) -> Vec<ImageView> {
    images
        .iter()
        .filter(|image| image.image_kind == kind)
        .map(image_view)
        .collect()
}

fn image_view(image: &StoredImage) -> ImageView {
    ImageView {
        url: format!("/api/v1/images/{}", image.relative_path),
        width: image.width,
        height: image.height,
    }
}

/// Runtime as a page shows it. Whole minutes, rounded rather than cut: a film
/// of a hundred and nineteen minutes and fifty seconds is two hours.
fn whole_minutes(duration: Millis) -> i64 {
    (duration.get() + 30_000) / 60_000
}

// ---------------------------------------------------------------------------
// Home
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct HomeParams {
    library: Option<String>,
}

#[derive(Debug, Serialize)]
struct HomeView {
    /// The few works the page opens on, largest of all.
    hero: Vec<HeroView>,
    /// Films this viewer started and has not finished, the latest first.
    carry_on: Vec<CarryOnView>,
    /// The episode each started series is waiting on.
    up_next: Vec<UpNextView>,
    /// What a home page leads with, newest first.
    recently_added: Vec<CardView>,
    /// One row per kind of library this server really holds.
    shelves: Vec<ShelfView>,
    works: i64,
    /// Whether anything is still waiting to be looked up, so the page can say
    /// so rather than showing untitled films with no explanation.
    awaiting_identification: i64,
}

/// One work the page opens on, and why it is there.
#[derive(Debug, Serialize)]
struct HeroView {
    #[serde(flatten)]
    card: CardView,
    /// started, pinned, new or suggested. What the button says is drawn from
    /// it, so the interface never has to work out again what the server
    /// already decided when it filled the row.
    because: &'static str,
    /// The wide picture behind it, and its title as its own designers drew
    /// it. Both empty for a work nobody has looked up, which the banner
    /// answers by writing the title out instead.
    backdrop: Vec<ImageView>,
    logo: Vec<ImageView>,
    tagline: Option<String>,
    overview: Option<String>,
    genres: Vec<String>,
    /// What the file itself holds, for the badges beside the title: how tall
    /// its picture is, which dynamic range it carries, and what its fullest
    /// soundtrack is. Raw as the file states it, because turning that into
    /// "4K" or "Dolby Atmos" is drawing rather than deciding.
    height: Option<i64>,
    hdr: Option<String>,
    sound: Option<String>,
    /// Set where the work shown large is an episode: the series it belongs to
    /// and where it sits in it. The picture and the drawn title above already
    /// come from that series, and these are what place it in words.
    series_title: Option<String>,
    season_number: Option<i32>,
    episode_number: Option<i32>,
}

/// One episode a started series is waiting on.
#[derive(Debug, Serialize)]
struct UpNextView {
    #[serde(flatten)]
    card: CardView,
    /// The series, which is what the card leads with: an episode's own title
    /// is not what anybody remembers a series by.
    series: String,
    series_title: String,
    season_number: Option<i32>,
    episode_number: Option<i32>,
}

/// One row of the home page, for one kind of library.
#[derive(Debug, Serialize)]
struct ShelfView {
    /// movies, series, anime or shows.
    kind: &'static str,
    /// The newest works of that kind, for the row of them further down the
    /// home page.
    cards: Vec<CardView>,
    /// The newest of them somebody named, for the fan of posters on the tile
    /// that leads to the kind.
    fan: Vec<CardView>,
}

/// One film somebody started and has not finished.
#[derive(Debug, Serialize)]
struct CarryOnView {
    #[serde(flatten)]
    card: CardView,
    /// Where they got to, so a card can draw how far in it is rather than
    /// asking again for every film in the row.
    position_seconds: f64,
    /// For an episode, the series it hangs under and where it sits in it.
    /// Nothing at all for a film. Nobody left off in the middle of an episode
    /// title, they left off in the middle of a series.
    series: Option<String>,
    series_title: Option<String>,
    season_number: Option<i32>,
    episode_number: Option<i32>,
}

async fn home(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Query(params): Query<HomeParams>,
) -> Result<Json<HomeView>> {
    let library_id = params.library.as_deref().map(parse_library).transpose()?;
    let page = melyxar_app::catalogue::home(&state, library_id, &who).await?;

    Ok(Json(HomeView {
        hero: page
            .hero
            .iter()
            .map(|entry| HeroView {
                card: card_view(&entry.card),
                because: entry.because.as_str(),
                backdrop: entry.dressed.backdrop.iter().map(image_view).collect(),
                logo: entry.dressed.logo.iter().map(image_view).collect(),
                tagline: entry.dressed.tagline.clone(),
                overview: entry.dressed.overview.clone(),
                genres: entry.dressed.genres.clone(),
                height: entry.dressed.height,
                hdr: entry.dressed.hdr.clone(),
                sound: entry.dressed.sound.clone(),
                series_title: entry
                    .episode_of
                    .as_ref()
                    .map(|place| place.series_title.clone()),
                season_number: entry.episode_of.as_ref().and_then(|place| place.season_number),
                episode_number: entry
                    .episode_of
                    .as_ref()
                    .and_then(|place| place.episode_number),
            })
            .collect(),
        carry_on: page
            .carry_on
            .iter()
            .map(|entry| CarryOnView {
                card: card_view(&entry.card),
                position_seconds: entry.position.get() as f64 / 1000.0,
                series: entry.series_id.map(|id| id.to_string()),
                series_title: entry.series_title.clone(),
                season_number: entry.season_number,
                episode_number: entry.episode_number,
            })
            .collect(),
        up_next: page
            .up_next
            .iter()
            .map(|entry| UpNextView {
                card: card_view(&entry.card),
                series: entry.series_id.to_string(),
                series_title: entry.series_title.clone(),
                season_number: entry.season_number,
                episode_number: entry.episode_number,
            })
            .collect(),
        recently_added: page.recently_added.cards.iter().map(card_view).collect(),
        shelves: page
            .shelves
            .iter()
            .map(|shelf| ShelfView {
                kind: shelf.kind.as_str(),
                cards: shelf.cards.iter().map(card_view).collect(),
                fan: shelf.fan.iter().map(card_view).collect(),
            })
            .collect(),
        works: page.works,
        awaiting_identification: page.awaiting_identification,
    }))
}

// ---------------------------------------------------------------------------
// One person
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct PersonView {
    id: String,
    name: String,
    biography: Option<String>,
    /// Year, month and day, as the provider writes them.
    born_on: Option<String>,
    died_on: Option<String>,
    birthplace: Option<String>,
    photo: Vec<ImageView>,
    /// What of theirs this viewer can open, newest first.
    works: Vec<CardView>,
}

/// The page of one person.
///
/// The first opening may wait on the provider, once: what it says of their
/// life is kept, and every opening after reads it from here.
async fn person(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<PersonView>> {
    let person_id = parse_person(&id)?;
    let page = melyxar_app::people::person_page(&state, &who, person_id)
        .await?
        .ok_or_else(|| ServerError::not_found("no person with that identifier"))?;

    Ok(Json(PersonView {
        id: page.person.id.to_string(),
        name: page.person.name,
        biography: page.person.biography,
        born_on: page.person.born_on,
        died_on: page.person.died_on,
        birthplace: page.person.birthplace,
        photo: page.photo.iter().map(image_view).collect(),
        works: page.works.iter().map(card_view).collect(),
    }))
}

// ---------------------------------------------------------------------------
// One work
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct WorkView {
    id: String,
    library_id: String,
    kind: &'static str,
    title: String,
    tagline: Option<String>,
    overview: Option<String>,
    year: Option<i32>,
    runtime_minutes: Option<i64>,
    rating: Option<f64>,
    age_rating: Option<String>,
    identification: &'static str,
    identification_note: Option<&'static str>,
    color: Option<String>,
    /// Which season or episode this is, for a page that draws its own number.
    number: Option<i32>,
    /// Whether the title above says anything its number does not, so a season
    /// a scan could only number is drawn in the language it is being read in.
    has_own_name: bool,
    genres: Vec<String>,
    studios: Vec<String>,
    collection: Option<String>,
    cast: Vec<CreditView>,
    crew: Vec<CreditView>,
    poster: Vec<ImageView>,
    backdrop: Vec<ImageView>,
    /// The title drawn as the film draws it, shown in place of the title
    /// written out. Empty for a film the provider draws under none.
    logo: Vec<ImageView>,
    versions: Vec<VersionView>,
    trailers: Vec<TrailerView>,
    external_ids: Vec<ExternalIdView>,
    /// The seasons of a series, the episodes of a season, in order. Empty for
    /// anything met on its own.
    children: Vec<ChildView>,
    /// The way back up, nearest first: an episode carries its season and then
    /// its series. Empty for anything met on its own.
    ancestry: Vec<AncestorView>,
    /// The episode this viewer would watch next, on the page of a series or a
    /// season. Absent when there is none left to watch.
    carry_on_with: Option<NextEpisodeView>,
    /// The episode before this one, so the player can offer to step back into
    /// it. Absent for anything that is not an episode, and for the first
    /// episode of a series.
    previous_episode: Option<NextEpisodeView>,
    /// The photos before and after this one in its folder. Absent for
    /// anything that is not a photo, and at either end of the folder.
    previous_photo: Option<String>,
    next_photo: Option<String>,
    /// This work as a card draws it, with what this viewer made of it: the
    /// page marks it through the same card every row shows.
    card: Option<CardView>,
    /// Works like this one by a genre they share, and that genre. Absent for
    /// anything but a film or a series, and when nothing shares a genre.
    alike: Option<AlikeView>,
}

/// A row of works like another, and the genre it was found by.
#[derive(Debug, Serialize)]
struct AlikeView {
    genre: String,
    cards: Vec<CardView>,
}

/// The episode a page offers to play next.
#[derive(Debug, Serialize)]
struct NextEpisodeView {
    id: String,
    season: Option<i32>,
    episode: Option<i32>,
    /// Its own name, when it has one its number does not already say.
    title: Option<String>,
    /// The file it plays from, so a button can start it without asking again.
    source_id: Option<String>,
}

/// One work hanging under this one, as a card on its page.
#[derive(Debug, Serialize)]
struct ChildView {
    id: String,
    /// season or episode.
    kind: &'static str,
    /// The season number, the episode number. Absent only if a row was ever
    /// written without one.
    number: Option<i32>,
    /// The name it carries, when that name says anything the number does not.
    /// Absent for a season a scan could only number, so a page draws the
    /// number in its own language rather than the English one written down.
    title: Option<String>,
    runtime_minutes: Option<i64>,
    /// How many episodes a season holds. Zero for an episode.
    child_count: i64,
    /// How many of those this viewer has left to watch.
    unwatched: i64,
    /// Whether this viewer has watched it. Only ever true of an episode.
    watched: bool,
    /// Where this viewer stopped in it, when they stopped partway.
    resume_from_seconds: Option<f64>,
    /// False when no file of it is on the disk right now, so a page can say so
    /// rather than offer it and fail.
    playable: bool,
    identification: &'static str,
    color: Option<String>,
    poster: Vec<ImageView>,
    /// The biggest copy on disk, so a row of these can start one playing on
    /// its own rather than sending a viewer to its page to ask again. Absent
    /// along with `playable`.
    source_id: Option<String>,
}

/// One work this one hangs under, as a way back to it.
#[derive(Debug, Serialize)]
struct AncestorView {
    id: String,
    kind: &'static str,
    number: Option<i32>,
    /// Absent for the same reason as above.
    title: Option<String>,
    /// The series' own mark, drawn as it draws its title. Empty for anything
    /// that is not a series.
    logo: Vec<ImageView>,
}

#[derive(Debug, Serialize)]
struct CreditView {
    /// Where their own page is.
    person_id: String,
    name: String,
    role: String,
    character: Option<String>,
    /// The face shown next to the name. Empty for anyone whose picture has not
    /// been fetched, and the page shows an initial instead.
    photo: Vec<ImageView>,
}

#[derive(Debug, Serialize)]
struct VersionView {
    id: String,
    /// The short line shown above a play button.
    summary: String,
    /// Where the file is, root included, and the disk it is on. On a page
    /// about a file, the first thing wanted when something is wrong with it
    /// is where it is.
    path: String,
    root_label: String,
    /// When a scan first saw it, which is not when the file was made.
    added_at: String,
    size_bytes: i64,
    duration_minutes: Option<i64>,
    /// Of the whole file, every track together.
    overall_bitrate: Option<i64>,
    container: Option<String>,
    /// False while nothing has looked inside the file yet.
    analysed: bool,
    /// True when the file is not on disk. Such a version is shown as
    /// unavailable rather than offered and failing when pressed.
    missing: bool,
    video: Vec<VideoTrackView>,
    audio: Vec<AudioTrackView>,
    subtitles: Vec<SubtitleTrackView>,
    chapters: usize,
}

#[derive(Debug, Serialize)]
struct VideoTrackView {
    /// The title the file itself carries, when it carries one.
    title: Option<String>,
    codec: String,
    profile: Option<String>,
    level: Option<i32>,
    width: i32,
    height: i32,
    aspect_ratio: Option<String>,
    is_interlaced: bool,
    /// hdr10, hlg or dolby_vision. Absent for an ordinary picture.
    hdr: Option<&'static str>,
    frame_rate: Option<f64>,
    bitrate: Option<i64>,
    pixel_format: Option<String>,
    reference_frames: Option<i32>,
    color_primaries: Option<String>,
    color_space: Option<String>,
    color_transfer: Option<String>,
    bit_depth: Option<i32>,
    is_default: bool,
}

#[derive(Debug, Serialize)]
struct AudioTrackView {
    title: Option<String>,
    codec: String,
    profile: Option<String>,
    language: Option<String>,
    channels: i32,
    channel_layout: Option<String>,
    sample_rate: Option<i32>,
    bit_depth: Option<i32>,
    bitrate: Option<i64>,
    is_default: bool,
    is_forced: bool,
}

#[derive(Debug, Serialize)]
struct SubtitleTrackView {
    title: Option<String>,
    codec: String,
    language: Option<String>,
    is_default: bool,
    is_forced: bool,
    is_hearing_impaired: bool,
    /// True for a file sitting next to the film rather than a stream inside it.
    is_external: bool,
    /// True when showing it means burning it into the picture, which costs a
    /// full conversion. Worth knowing before pressing play.
    burns_in: bool,
}

#[derive(Debug, Serialize)]
struct TrailerView {
    name: Option<String>,
    /// Set when playing it leaves this server.
    remote_url: Option<String>,
    /// Set when the file sits next to the film.
    local: bool,
    /// Where to fetch a local one. Absent for a link, which is watched where
    /// it lives: this server does not go and fetch someone else's video.
    url: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExternalIdView {
    provider: String,
    id: String,
}

async fn work(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<WorkView>> {
    let work_id = parse_work(&id)?;
    let detail = melyxar_app::detail::work_detail(&state, &who, work_id)
        .await?
        .ok_or_else(|| ServerError::not_found("no work with that identifier"))?;

    Ok(Json(work_view(&detail)))
}

/// What a viewer thinks of a film, and what they are saying about it now.
#[derive(Debug, Deserialize)]
struct FavouriteBody {
    favourite: bool,
}

/// What it is now, which is what a button is drawn from.
#[derive(Debug, Serialize)]
struct FavouriteView {
    favourite: bool,
}

/// Marks a film as one this viewer likes, or takes the mark off.
///
/// Answers the state it is in now rather than nothing at all: the button is
/// drawn from the answer, so a viewer pressing it twice in a second cannot end
/// up with a button saying one thing and the server another.
async fn set_favourite(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Json(body): Json<FavouriteBody>,
) -> Result<Json<FavouriteView>> {
    let work_id = parse_work(&id)?;
    let favourite =
        melyxar_app::playback::set_favourite(&state, &who, work_id, body.favourite).await?;
    Ok(Json(FavouriteView { favourite }))
}

/// Whether this work is to stand on the server's own shelf.
#[derive(Debug, Deserialize)]
struct PinnedBody {
    pinned: bool,
}

/// What it is now, which is what the menu entry is drawn from.
#[derive(Debug, Serialize)]
struct PinnedView {
    pinned: bool,
}

/// Puts a work in front of everybody, or takes it back off.
///
/// Asks for an administrator, which is the whole difference between this and
/// a favourite: one is the server saying what to watch, the other is a person
/// saying what they liked.
async fn set_pinned(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    _: crate::account::Administrator,
    Path(id): Path<String>,
    Json(body): Json<PinnedBody>,
) -> Result<Json<PinnedView>> {
    let work_id = parse_work(&id)?;
    let pinned = melyxar_app::catalogue::set_pinned(&state, &who, work_id, body.pinned).await?;
    Ok(Json(PinnedView { pinned }))
}

/// Whether this viewer is saying they have watched it.
#[derive(Debug, Deserialize)]
struct WatchedBody {
    watched: bool,
}

/// What it is now, which is what the tick on the card is drawn from.
#[derive(Debug, Serialize)]
struct WatchedView {
    watched: bool,
}

/// Marks a work watched by hand, or puts it back to unwatched.
///
/// The tick on a card is pressed directly, so this answers the state it is in
/// now for the same reason the favourite does: a card is drawn from the
/// answer, not from what was asked for.
async fn set_watched(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Json(body): Json<WatchedBody>,
) -> Result<Json<WatchedView>> {
    let work_id = parse_work(&id)?;
    let watched = melyxar_app::playback::mark_watched(&state, &who, work_id, body.watched).await?;
    Ok(Json(WatchedView { watched }))
}

fn work_view(detail: &WorkDetail) -> WorkView {
    let images_of = |kind: &str| pictures_of(&detail.images, kind);

    WorkView {
        id: detail.work.id.to_string(),
        library_id: detail.work.library_id.to_string(),
        kind: detail.work.kind.as_str(),
        title: detail.work.title.clone(),
        tagline: detail.tagline.clone(),
        overview: detail.overview.clone(),
        year: detail.work.release_year,
        // The runtime of the film, not the length of any one copy of it. A
        // file a few seconds short is still that film, and a page that shows
        // the file length shows a different number for each version.
        runtime_minutes: detail.work.runtime.map(whole_minutes),
        rating: detail.work.community_rating,
        age_rating: detail.work.age_rating_label.clone(),
        identification: detail.work.identification.as_str(),
        identification_note: detail
            .work
            .identification_note
            .map(IdentificationNote::as_str),
        color: detail.work.dominant_color.clone(),
        number: detail.work.ordinal,
        has_own_name: its_own_name(detail.work.kind, detail.work.ordinal, &detail.work.title)
            .is_some(),
        genres: detail.genres.clone(),
        studios: detail.studios.clone(),
        collection: detail.collection.clone(),
        cast: detail
            .credits
            .iter()
            .filter(|credit| credit.role == "actor")
            .map(credit_view)
            .collect(),
        crew: detail
            .credits
            .iter()
            .filter(|credit| credit.role != "actor")
            .map(credit_view)
            .collect(),
        poster: images_of("poster"),
        backdrop: images_of("backdrop"),
        logo: images_of("logo"),
        carry_on_with: detail.carry_on_with.as_ref().map(next_episode_view),
        previous_episode: detail.previous_episode.as_ref().map(next_episode_view),
        previous_photo: detail.previous_photo.map(|id| id.to_string()),
        next_photo: detail.next_photo.map(|id| id.to_string()),
        card: detail.card.as_ref().map(card_view),
        alike: detail.alike.as_ref().map(|alike| AlikeView {
            genre: alike.genre.clone(),
            cards: alike.cards.iter().map(card_view).collect(),
        }),
        children: detail.children.iter().map(child_view).collect(),
        ancestry: detail
            .ancestry
            .iter()
            .map(|up| AncestorView {
                id: up.id.to_string(),
                kind: up.kind.as_str(),
                number: up.ordinal,
                title: its_own_name(up.kind, up.ordinal, &up.title),
                logo: pictures_of(&up.logo, "logo"),
            })
            .collect(),
        versions: detail.versions.iter().map(version_view).collect(),
        trailers: detail
            .trailers
            .iter()
            .enumerate()
            .map(|(rank, trailer)| TrailerView {
                name: trailer.name.clone(),
                remote_url: trailer.remote_url.clone(),
                local: trailer.local.is_some(),
                // Named by its place in the list this very answer carries,
                // which is what keeps a path on someone's disk out of a page
                // and out of anything a client could send back.
                url: trailer
                    .local
                    .is_some()
                    .then(|| format!("/api/v1/works/{}/trailers/{rank}", detail.work.id)),
            })
            .collect(),
        external_ids: detail
            .external_ids
            .iter()
            .map(|(provider, id)| ExternalIdView {
                provider: provider.clone(),
                id: id.clone(),
            })
            .collect(),
    }
}

fn credit_view(credit: &Credit) -> CreditView {
    CreditView {
        person_id: credit.person_id.to_string(),
        name: credit.name.clone(),
        role: credit.role.clone(),
        character: credit.character.clone(),
        photo: credit.photo.iter().map(image_view).collect(),
    }
}

fn version_view(version: &Version) -> VersionView {
    let mut video = Vec::new();
    let mut audio = Vec::new();
    let mut subtitles = Vec::new();

    for track in &version.tracks {
        match &track.kind {
            TrackKind::Video(details) => video.push(VideoTrackView {
                title: track.title.clone(),
                codec: details.codec.clone(),
                profile: details.profile.clone(),
                level: details.level,
                // The picture rather than the frame it sits in: a film
                // that says part of its frame is not the picture is shown by
                // its picture, and that is what anybody reading this means by
                // the size of a film.
                width: details.visible_width(),
                height: details.visible_height(),
                aspect_ratio: details.aspect_ratio.clone(),
                is_interlaced: details.is_interlaced,
                hdr: details.hdr.map(|hdr| hdr.as_word()),
                frame_rate: details.frame_rate,
                bitrate: details.bitrate,
                pixel_format: details.pixel_format.clone(),
                reference_frames: details.reference_frames,
                color_primaries: details.color.primaries.clone(),
                color_space: details.color.space.clone(),
                color_transfer: details.color.transfer.clone(),
                bit_depth: details.color.bit_depth,
                is_default: track.is_default,
            }),
            TrackKind::Audio(details) => audio.push(AudioTrackView {
                title: track.title.clone(),
                codec: details.codec.clone(),
                profile: details.profile.clone(),
                language: track.language.clone(),
                channels: details.channels,
                channel_layout: details.channel_layout.clone(),
                sample_rate: details.sample_rate,
                bit_depth: details.bit_depth,
                bitrate: details.bitrate,
                is_default: track.is_default,
                is_forced: track.is_forced,
            }),
            TrackKind::Subtitle(details) => subtitles.push(SubtitleTrackView {
                title: track.title.clone(),
                codec: details.codec.clone(),
                language: track.language.clone(),
                is_default: track.is_default,
                is_forced: track.is_forced,
                is_hearing_impaired: details.is_hearing_impaired,
                is_external: details.is_external,
                burns_in: details.forces_full_transcode(),
            }),
        }
    }

    VersionView {
        id: version.source_id.to_string(),
        summary: version.summary(),
        path: version.path.clone(),
        root_label: version.root_label.clone(),
        added_at: melyxar_core::time::to_text(version.added_at),
        size_bytes: version.size_bytes,
        duration_minutes: version.duration.map(whole_minutes),
        overall_bitrate: version.overall_bitrate,
        container: version.container.clone(),
        analysed: version.analysed,
        missing: version.missing_since.is_some(),
        chapters: version.chapters.len(),
        video,
        audio,
        subtitles,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::id::{MediaSourceId, TrackId};
    use melyxar_core::media::{
        AudioDetails, ColorInfo, HdrFormat, SubtitleDetails, SubtitleLayout, Track, VideoDetails,
    };

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        // Built at start-up, so a route a router refuses brings the whole
        // server down rather than failing one request.
        let _ = router();
    }

    #[test]
    fn what_a_viewer_says_about_liking_a_film_is_read_whole() {
        let said: FavouriteBody = serde_json::from_str(r#"{"favourite":true}"#).expect("read");
        assert!(said.favourite);
        let said: FavouriteBody = serde_json::from_str(r#"{"favourite":false}"#).expect("read");
        assert!(!said.favourite);
        // Anything else is refused rather than read as one or the other.
        assert!(serde_json::from_str::<FavouriteBody>(r#"{}"#).is_err());
    }

    fn version_with(tracks: Vec<Track>) -> Version {
        Version {
            source_id: MediaSourceId::new(),
            root_label: "disk-one".to_string(),
            path: "/mnt/disk-one/films/a.mkv".to_string(),
            added_at: melyxar_core::time::now(),
            relative_path: "Quiet.Harbour.2019.mkv".to_string(),
            size_bytes: 12_000_000_000,
            missing_since: None,
            container: Some("matroska,webm".to_string()),
            duration: Some(Millis::new(7_190_000)),
            overall_bitrate: None,
            analysed: true,
            tracks,
            chapters: Vec::new(),
        }
    }

    fn wide_gamut_video() -> Track {
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
                width: 3840,
                height: 2160,
                margins: None,
                aspect_ratio: None,
                is_interlaced: false,
                frame_rate: Some(23.976),
                bitrate: None,
                pixel_format: None,
                reference_frames: None,
                color: ColorInfo::default(),
                hdr: Some(HdrFormat::DolbyVision { profile: Some(5) }),
            }),
        }
    }

    fn picture_subtitle() -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: 1,
            language: Some("fre".to_string()),
            title: None,
            is_default: false,
            is_forced: true,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: "hdmv_pgs_subtitle".to_string(),
                layout: SubtitleLayout::Bitmap,
                is_hearing_impaired: false,
                is_external: false,
                external_relative_path: None,
            }),
        }
    }

    fn soundtrack() -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: 2,
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

    #[test]
    fn a_version_carries_what_the_page_needs_to_warn_about() {
        let view = version_view(&version_with(vec![
            wide_gamut_video(),
            soundtrack(),
            picture_subtitle(),
        ]));

        assert_eq!(view.video[0].hdr, Some("dolby_vision"));
        assert!(
            view.subtitles[0].burns_in,
            "a viewer should know before pressing play that this one costs a full conversion"
        );
        assert_eq!(view.audio[0].channel_layout.as_deref(), Some("5.1"));
        assert!(!view.missing);
    }

    #[test]
    fn a_runtime_is_rounded_the_way_a_person_would_round_it() {
        assert_eq!(whole_minutes(Millis::new(7_190_000)), 120);
        assert_eq!(whole_minutes(Millis::new(60_000)), 1);
        assert_eq!(whole_minutes(Millis::new(29_000)), 0);
    }

    #[test]
    fn a_file_that_is_gone_is_shown_as_gone_rather_than_offered() {
        let mut version = version_with(vec![wide_gamut_video()]);
        version.missing_since = Some(melyxar_core::time::now());
        assert!(version_view(&version).missing);
    }

    #[test]
    fn an_image_is_addressed_by_a_name_that_changes_with_its_content() {
        let image = StoredImage {
            owner_kind: "work".to_string(),
            owner_id: "w".to_string(),
            image_kind: "poster".to_string(),
            relative_path: "works/w/poster-abc123-400.webp".to_string(),
            width: Some(400),
            height: Some(600),
            fingerprint: "abc123".to_string(),
            dominant_color: None,
        };
        assert_eq!(
            image_view(&image),
            ImageView {
                url: "/api/v1/images/works/w/poster-abc123-400.webp".to_string(),
                width: Some(400),
                height: Some(600),
            }
        );
    }

    #[test]
    fn each_field_of_a_page_takes_the_kind_of_picture_it_shows() {
        let picture = |kind: &str, width: i32| StoredImage {
            owner_kind: "work".to_string(),
            owner_id: "w".to_string(),
            image_kind: kind.to_string(),
            relative_path: format!("works/w/{kind}-abc123-{width}.webp"),
            width: Some(width),
            height: Some(width / 2),
            fingerprint: "abc123".to_string(),
            dominant_color: None,
        };
        let held = [
            picture("poster", 400),
            picture("logo", 340),
            picture("logo", 680),
            picture("backdrop", 1280),
        ];

        let titles = pictures_of(&held, "logo");
        assert_eq!(titles.len(), 2);
        assert_eq!(titles[0].url, "/api/v1/images/works/w/logo-abc123-340.webp");
        assert_eq!(
            pictures_of(&held, "poster").len(),
            1,
            "a title image among the posters would be a card with a wordmark on it"
        );
        assert!(pictures_of(&held, "photo").is_empty());
    }

    #[test]
    fn a_malformed_identifier_is_refused_rather_than_looked_up() {
        assert!(parse_work("not-an-identifier").is_err());
        assert!(parse_library("").is_err());
    }
}
