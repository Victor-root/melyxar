//! Fetching the pictures of a work and preparing what the interface serves.
//!
//! Three rules, all of them about not doing the same work twice. Sizes are
//! generated once and never on demand, because a server that resizes a poster
//! per request spends its afternoon resizing the same poster. A picture is
//! served under a name earned by its content, so a browser keeps it for ever
//! and still sees a new one the day it changes. And a picture whose content
//! has not changed is not fetched again at all.
//!
//! A picture that will not come is never a failure of the film: the work is
//! identified, the card shows its colour, and the picture arrives another day.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use melyxar_core::fingerprint;
use melyxar_core::id::WorkId;
use melyxar_core::orientation::Orientation;
use melyxar_core::time::Millis;
use melyxar_database::images::StoredImage;
use melyxar_database::metadata::CreditedPerson;
use melyxar_metadata::{Details, MetadataProvider, PictureKind};

use crate::{AppState, Result};

/// How the pictures served are made, written in front of every fingerprint.
///
/// A fingerprint says "this picture is already prepared, leave it alone". It
/// was made of what the provider called the picture, which answers whether the
/// picture changed but not whether the way it is prepared did. So a library
/// filled before a rule like "never enlarge a picture" kept for ever what that
/// rule was written to stop.
///
/// Standing in front of the fingerprint, this makes every picture prepared by
/// an older recipe differ from the one wanted now: it is made again, once, the
/// next time the work is looked at, and never again after that.
///
/// Raise it when what is written out changes, never for anything else.
pub const RECIPE: &str = "b5";

/// What a picture is for, which decides its widths and where it is filed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Poster,
    Backdrop,
    /// The film's title drawn as the film draws it, shown in place of the
    /// title written out.
    Logo,
    /// Wide, with the title written on it, for a card lying on its side.
    Thumb,
    Photo,
}

impl Kind {
    /// The one a caller outside this file names. A face belongs to a person
    /// rather than to a work, and nobody chooses one by hand, so the kinds
    /// that can be chosen are the four a work wears.
    fn of(kind: PictureKind) -> Self {
        match kind {
            PictureKind::Poster => Self::Poster,
            PictureKind::Backdrop => Self::Backdrop,
            PictureKind::Logo => Self::Logo,
            PictureKind::Thumb => Self::Thumb,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Poster => "poster",
            Self::Backdrop => "backdrop",
            Self::Logo => "logo",
            Self::Thumb => "thumb",
            Self::Photo => "photo",
        }
    }

    /// Whether the colour of this picture is what a card is painted with.
    fn carries_the_colour_of_its_work(self) -> bool {
        matches!(self, Self::Poster)
    }

    fn widths(self) -> &'static [u32] {
        match self {
            Self::Poster => &melyxar_ffmpeg::images::POSTER_WIDTHS,
            Self::Backdrop | Self::Thumb => &melyxar_ffmpeg::images::BACKDROP_WIDTHS,
            Self::Logo => &melyxar_ffmpeg::images::LOGO_WIDTHS,
            Self::Photo => &melyxar_ffmpeg::images::PHOTO_WIDTHS,
        }
    }

    /// What the picture belongs to: a film, or a person who is in several.
    fn owner_kind(self) -> &'static str {
        match self {
            Self::Poster | Self::Backdrop | Self::Logo | Self::Thumb => "work",
            Self::Photo => "person",
        }
    }

    /// The folder of the cache it is filed under.
    fn folder(self) -> &'static str {
        match self {
            Self::Poster | Self::Backdrop | Self::Logo | Self::Thumb => "works",
            Self::Photo => "people",
        }
    }
}

/// Fetches the pictures a provider named for one work.
///
/// Answers how many pictures were prepared. Nothing here can fail the caller:
/// a picture is a comfort, and a film without one is still a film.
pub async fn store_provider_images(
    state: &AppState,
    provider: &impl MetadataProvider,
    work_id: WorkId,
    details: &Details,
) -> usize {
    let wanted = [
        (Kind::Poster, details.poster_path.as_deref()),
        (Kind::Backdrop, details.backdrop_path.as_deref()),
        (Kind::Logo, details.logo_path.as_deref()),
        (Kind::Thumb, details.thumb_path.as_deref()),
    ];
    // Said out loud rather than passed over: a work with no picture is
    // indistinguishable from one whose picture failed to arrive, and the two
    // want opposite answers. One work is one card, so this is a handful of
    // lines per scan, which is why it is said here and not for a season or an
    // episode, where it would be one line per file of the collection.
    for (kind, _) in wanted.iter().filter(|(_, path)| path.is_none()) {
        tracing::info!(
            work = %details.title,
            kind = kind.as_str(),
            "the provider named no picture of this kind for this work"
        );
    }
    store(state, provider, work_id, &wanted).await
}

/// Fetches the one picture a season or an episode has.
///
/// A provider describes a part of a series with a picture and nothing else:
/// there is no backdrop of a season and no title drawn for an episode. Asking
/// for the three would not fetch anything more; it would only say, of every
/// season and every episode of the collection, that two pictures which cannot
/// exist did not arrive.
///
/// Says nothing when the provider has no picture either, for the same reason:
/// a long run whose episodes it never illustrated is one line per episode, and
/// the series says it once for all of them.
pub async fn store_provider_poster(
    state: &AppState,
    provider: &impl MetadataProvider,
    work_id: WorkId,
    details: &Details,
) -> usize {
    store(
        state,
        provider,
        work_id,
        &[(Kind::Poster, details.poster_path.as_deref())],
    )
    .await
}

async fn store(
    state: &AppState,
    provider: &impl MetadataProvider,
    work_id: WorkId,
    wanted: &[(Kind, Option<&str>)],
) -> usize {
    let Some(tools) = state.tools() else {
        return 0;
    };

    // What somebody chose by hand, which no run of this undoes. A picture put
    // there deliberately and swept away by the next refresh is the whole
    // reason a person stops trusting a server with their library.
    let by_hand = state
        .database()
        .locked_fields(work_id)
        .await
        .unwrap_or_default();

    let owner_id = work_id.to_db_string();
    let mut prepared = 0;
    for (kind, path) in wanted.iter().copied() {
        let Some(path) = path else {
            continue;
        };
        if by_hand.iter().any(|field| field == kind.as_str()) {
            continue;
        }
        match store_one(state, provider, &tools.ffmpeg, kind, &owner_id, path).await {
            Ok(None) => {}
            Ok(Some(picture)) => {
                prepared += 1;
                if kind == Kind::Poster {
                    remember_the_colour(state, work_id, picture.colour).await;
                }
            }
            Err(error) => {
                tracing::warn!(
                    kind = kind.as_str(),
                    error = %error,
                    "a picture could not be prepared; the work keeps everything else"
                );
            }
        }
    }
    prepared
}

/// Paints a card with the colour of its poster.
///
/// The card shows a colour before any picture arrives, and the poster is what
/// a card shows, so it is the poster that gives the colour.
async fn remember_the_colour(state: &AppState, work_id: WorkId, colour: Option<String>) {
    let Some(colour) = colour else {
        return;
    };
    if let Err(error) = state
        .database()
        .set_work_dominant_color(work_id, &colour)
        .await
    {
        tracing::warn!(error = %error, "the colour of a card could not be recorded");
    }
}

/// Prepares the picture of a file somebody filmed or photographed themselves,
/// out of the file itself.
///
/// What it is made from is said by `made_from`, which changes whenever the
/// file does, so a file already pictured is not pictured again. Answers
/// whether a picture was made.
pub(crate) async fn store_own_picture(
    state: &AppState,
    work_id: WorkId,
    file: &Path,
    at: Option<Millis>,
    orientation: Orientation,
    made_from: &str,
) -> Result<bool> {
    let Some(tools) = state.tools() else {
        return Ok(false);
    };
    let kind = Kind::Poster;
    let owner_id = work_id.to_db_string();
    let fingerprint = stamp(made_from);
    if state
        .database()
        .image_fingerprint(kind.owner_kind(), &owner_id, kind.as_str())
        .await?
        .as_deref()
        == Some(fingerprint.as_str())
    {
        return Ok(false);
    }

    let folder = state
        .config()
        .directories
        .images()
        .join(kind.folder())
        .join(&owner_id);
    tokio::fs::create_dir_all(&folder).await?;
    let original = folder.join(format!("{}-{fingerprint}.png", kind.as_str()));
    melyxar_ffmpeg::images::upright_picture(&tools.ffmpeg, file, at, orientation, &original)
        .await?;
    let written = write_every_size(
        state,
        &tools.ffmpeg,
        kind,
        &owner_id,
        &fingerprint,
        &original,
    )
    .await;
    tokio::fs::remove_file(&original).await.ok();

    let Some(picture) = written? else {
        return Ok(false);
    };
    remember_the_colour(state, work_id, picture.colour).await;
    Ok(true)
}

/// Every picture the provider holds for one work, to be chosen among by hand.
///
/// Asked for only when somebody opens the panel that chooses: it is a request
/// of its own, and nothing else in the server has any use for the whole set.
pub async fn offered_pictures(
    state: &AppState,
    provider: &impl MetadataProvider,
    work_id: WorkId,
) -> Result<Vec<melyxar_metadata::OfferedPicture>> {
    let work = state
        .database()
        .work(work_id)
        .await?
        .ok_or_else(|| crate::AppError::Domain(melyxar_core::Error::not_found("work")))?;
    let catalogue = melyxar_metadata::Catalogue::of(work.kind)
        .ok_or_else(|| crate::AppError::Domain(melyxar_core::Error::not_found("catalogue")))?;
    let named = state
        .database()
        .work_external_ids(work_id)
        .await?
        .into_iter()
        .find(|(who, _)| who == provider.name())
        .map(|(_, id)| id)
        .ok_or_else(|| {
            crate::AppError::Domain(melyxar_core::Error::invalid_input(
                "this work has not been identified, so there is nothing to offer",
            ))
        })?;

    let language = state
        .database()
        .list_libraries()
        .await?
        .into_iter()
        .find(|library| library.id == work.library_id)
        .map(|library| library.metadata_language)
        .unwrap_or_else(|| "en".to_string());

    provider
        .pictures(catalogue, &named, &language)
        .await
        .map_err(|error| {
            crate::AppError::Domain(melyxar_core::Error::invalid_input(error.to_string()))
        })
}

/// Puts one picture chosen by hand on a work, in place of whatever was there.
///
/// Remembered as a choice: the kind is locked, so no later run replaces it.
/// What a person chose looking at the work is worth more than what a rule
/// chose looking at a name, and a picture swept away by the next refresh is
/// how somebody stops trusting a server with their library.
pub async fn choose_picture(
    state: &AppState,
    provider: &impl MetadataProvider,
    work_id: WorkId,
    kind: PictureKind,
    provider_path: &str,
) -> Result<bool> {
    let Some(tools) = state.tools() else {
        return Ok(false);
    };
    let kind = Kind::of(kind);
    let owner_id = work_id.to_db_string();

    let prepared = store_one(
        state,
        provider,
        &tools.ffmpeg,
        kind,
        &owner_id,
        provider_path,
    )
    .await?;

    state.database().lock_field(work_id, kind.as_str()).await?;
    if kind == Kind::Poster {
        if let Some(colour) = prepared.as_ref().and_then(|picture| picture.colour.clone()) {
            state
                .database()
                .set_work_dominant_color(work_id, &colour)
                .await?;
        }
    }
    Ok(prepared.is_some())
}

/// Takes a picture off a work, files and all.
///
/// Remembered like a choice, and for the same reason: somebody who took a
/// picture off did not want it, and a refresh that puts it back is a refresh
/// arguing with them.
pub async fn forget_picture(state: &AppState, work_id: WorkId, kind: PictureKind) -> Result<()> {
    let kind = Kind::of(kind);
    let root = state.config().directories.images();
    let no_longer_used = state
        .database()
        .replace_images(kind.owner_kind(), &work_id.to_db_string(), kind.as_str(), &[])
        .await?;
    for path in no_longer_used {
        tokio::fs::remove_file(root.join(path)).await.ok();
    }
    state.database().lock_field(work_id, kind.as_str()).await?;
    Ok(())
}

/// How many faces are worth fetching for one work.
///
/// Exactly what the cast row of a detail page shows. Fewer would leave empty
/// circles among the faces; more would mean fetching pictures for names a
/// provider lists and nobody displays, which for a long cast is fifty
/// pictures per film.
const FACES_FETCHED: usize = 18;

/// Fetches the faces a page shows next to the cast.
///
/// Only the actors, and only the first of them: nobody scrolls to the
/// forty-third name, and a face already fetched for another film is never
/// fetched again, since people are shared.
pub async fn store_person_photos<P>(
    state: &AppState,
    provider: &Arc<P>,
    people: &[CreditedPerson],
) -> usize
where
    P: MetadataProvider + 'static,
{
    let Some(tools) = state.tools() else {
        return 0;
    };

    let mut cast: Vec<&CreditedPerson> = people
        .iter()
        .filter(|person| person.role == "actor" && person.photo_path.is_some())
        .collect();
    cast.sort_by_key(|person| person.ordinal);

    let wanted: Vec<(String, String)> = cast
        .into_iter()
        .take(FACES_FETCHED)
        .map(|person| {
            (
                person.person_id.to_db_string(),
                person.photo_path.clone().unwrap_or_default(),
            )
        })
        .collect();
    if wanted.is_empty() {
        return 0;
    }

    // A film brings up to eighteen faces, and a face is mostly a wait on
    // somebody else's server. Fetched one after another they decide how long
    // naming a whole library takes, so several are in flight at once, bounded
    // by what the configuration allows for picture work: the bound is what
    // keeps a first fill from taking the machine away from whoever is watching
    // something.
    let limit = state.config().limits.concurrent_image_jobs;
    let owned_state = state.clone();
    let tool = tools.ffmpeg.clone();
    let shared = Arc::clone(provider);

    let prepared = melyxar_jobs::for_each_bounded(wanted, limit, move |(owner_id, path)| {
        let state = owned_state.clone();
        let tool = tool.clone();
        let provider = Arc::clone(&shared);
        async move {
            match store_one(
                &state,
                provider.as_ref(),
                &tool,
                Kind::Photo,
                &owner_id,
                &path,
            )
            .await
            {
                Ok(Some(_)) => 1,
                Ok(None) => 0,
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        "a face could not be prepared; the page shows the name on its own"
                    );
                    0
                }
            }
        }
    })
    .await;

    prepared.iter().sum()
}

/// What preparing one picture left behind.
struct Prepared {
    /// The average colour of the picture, when the tool could read one.
    colour: Option<String>,
}

/// Prepares one picture, and answers with nothing when there was nothing to do.
async fn store_one(
    state: &AppState,
    provider: &impl MetadataProvider,
    tool: &Path,
    kind: Kind,
    owner_id: &str,
    provider_path: &str,
) -> Result<Option<Prepared>> {
    let database = state.database();
    // The provider changes the path when it changes the picture, so the path
    // already says whether anything is new. Hashing the bytes would mean
    // fetching them first, which is the very thing to avoid.
    let fingerprint = stamp(provider_path);

    if database
        .image_fingerprint(kind.owner_kind(), owner_id, kind.as_str())
        .await?
        .as_deref()
        == Some(fingerprint.as_str())
    {
        return Ok(None);
    }

    let bytes = match provider.fetch_image(provider_path).await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(
                kind = kind.as_str(),
                reason = %error,
                "the picture could not be fetched; it will be asked for again later"
            );
            return Ok(None);
        }
    };

    let folder = state
        .config()
        .directories
        .images()
        .join(kind.folder())
        .join(owner_id);
    tokio::fs::create_dir_all(&folder).await?;

    // The picture as it arrived is kept only while the sizes are made from it.
    let original = folder.join(format!("{}-{fingerprint}.source", kind.as_str()));
    tokio::fs::write(&original, &bytes).await?;
    let written = write_every_size(state, tool, kind, owner_id, &fingerprint, &original).await;
    tokio::fs::remove_file(&original).await.ok();
    written
}

/// Writes one picture at every width the interface serves for its kind, and
/// keeps them in place of the ones it had.
///
/// Answers with nothing when no size could be written. The picture itself is
/// left where it is, for whoever made it to take away.
async fn write_every_size(
    state: &AppState,
    tool: &Path,
    kind: Kind,
    owner_id: &str,
    fingerprint: &str,
    original: &Path,
) -> Result<Option<Prepared>> {
    let database = state.database();
    let root = state.config().directories.images();
    let folder = root.join(kind.folder()).join(owner_id);

    // Read only where it is used. A card is painted with the colour of its
    // poster; nothing anywhere shows the average colour of a backdrop or of a
    // face, and reading one costs a run of the tool per picture.
    let colour = match kind.carries_the_colour_of_its_work() {
        true => melyxar_ffmpeg::images::average_colour(tool, original)
            .await
            .ok(),
        false => None,
    };
    let source_size = source_dimensions(state, original).await;

    let names: Vec<(u32, String)> = widths_worth_writing(kind.widths(), source_size.map(|(w, _)| w))
        .iter()
        .map(|width| {
            (
                *width,
                format!("{}-{fingerprint}-{width}.webp", kind.as_str()),
            )
        })
        .collect();
    let destinations: Vec<(u32, PathBuf)> = names
        .iter()
        .map(|(width, name)| (*width, folder.join(name)))
        .collect();
    let borrowed: Vec<(u32, &Path)> = destinations
        .iter()
        .map(|(width, path)| (*width, path.as_path()))
        .collect();

    let mut prepared = Vec::new();
    match melyxar_ffmpeg::images::resize(tool, original, &borrowed).await {
        Ok(()) => {
            for (width, name) in &names {
                prepared.push(StoredImage {
                    owner_kind: kind.owner_kind().to_string(),
                    owner_id: owner_id.to_string(),
                    image_kind: kind.as_str().to_string(),
                    relative_path: format!("{}/{owner_id}/{name}", kind.folder()),
                    width: Some(*width as i32),
                    height: source_size.map(|(w, h)| scaled_height(*width, w, h)),
                    fingerprint: fingerprint.to_string(),
                    dominant_color: colour.clone(),
                });
            }
        }
        Err(error) => {
            tracing::warn!(
                kind = kind.as_str(),
                error = %error,
                "a picture could not be written in the sizes the interface serves"
            );
        }
    }

    if prepared.is_empty() {
        return Ok(None);
    }

    let no_longer_used = database
        .replace_images(kind.owner_kind(), owner_id, kind.as_str(), &prepared)
        .await?;
    for path in no_longer_used {
        tokio::fs::remove_file(root.join(path)).await.ok();
    }

    Ok(Some(Prepared { colour }))
}

/// What stands beside a picture to say it is already prepared: the recipe
/// that made it, and what the provider called it.
fn stamp(provider_path: &str) -> String {
    format!("{RECIPE}-{}", fingerprint::of_text(provider_path))
}

/// The widths worth writing for a picture that arrived this big.
///
/// Never one larger than what arrived. Enlarging a picture invents nothing: it
/// writes a file four times the size holding the same detail, and, worse, the
/// interface is then told that a picture that wide exists. Asking in good
/// faith for the widest it is offered, a browser takes the enlargement and
/// draws it across the whole banner, where being soft is most visible. The
/// banner of a film whose picture the provider holds at half that width was
/// exactly that, and looked it.
///
/// So each width wanted is brought down to what the picture really is, and the
/// duplicates that makes are dropped: a picture of twelve hundred points asked
/// for at six hundred and forty, twelve hundred and eighty and nineteen twenty
/// is written twice, at six hundred and forty and at twelve hundred.
///
/// A picture whose size could not be read keeps every width wanted: guessing
/// it small would throw away detail that is really there.
fn widths_worth_writing(wanted: &[u32], source_width: Option<i32>) -> Vec<u32> {
    let Some(source) = source_width.filter(|width| *width > 0).map(|width| width as u32) else {
        return wanted.to_vec();
    };
    let mut kept: Vec<u32> = wanted.iter().map(|width| (*width).min(source)).collect();
    kept.sort_unstable();
    kept.dedup();
    kept
}

/// The size of the picture as it arrived, so a client can leave the right
/// space for it before it loads.
async fn source_dimensions(state: &AppState, path: &Path) -> Option<(i32, i32)> {
    let tools = state.tools()?;
    let report = melyxar_ffmpeg::probe::probe(&tools.ffprobe, path)
        .await
        .ok()?;
    let stream = report.streams.first()?;
    usable_dimensions(stream.width, stream.height)
}

/// The size of a picture, when the tool gave one worth using.
///
/// A side of nothing is not a size: a client told a picture is zero wide
/// leaves no room for it, and every height computed from it is zero.
fn usable_dimensions(width: Option<i32>, height: Option<i32>) -> Option<(i32, i32)> {
    match (width, height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => Some((width, height)),
        _ => None,
    }
}

/// The height a width implies, keeping the shape of the picture.
fn scaled_height(width: u32, source_width: i32, source_height: i32) -> i32 {
    if source_width <= 0 {
        return 0;
    }
    ((width as i64 * source_height as i64) / source_width as i64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_with_a_side_of_nothing_has_no_size_worth_using() {
        // A tool that answers zero, or answers nothing at all, must not end up
        // as a size: a client told a picture is zero wide leaves no room for
        // it, and every height computed from it is zero.
        assert_eq!(usable_dimensions(Some(600), Some(900)), Some((600, 900)));
        assert_eq!(usable_dimensions(Some(0), Some(900)), None);
        assert_eq!(usable_dimensions(Some(600), Some(0)), None);
        assert_eq!(usable_dimensions(None, Some(900)), None);
        assert_eq!(usable_dimensions(Some(600), None), None);
        assert_eq!(usable_dimensions(Some(-1), Some(900)), None);
    }

    #[test]
    fn no_picture_is_ever_written_larger_than_it_arrived() {
        let wanted = melyxar_ffmpeg::images::BACKDROP_WIDTHS;

        assert_eq!(
            widths_worth_writing(&wanted, Some(3840)),
            wanted.to_vec(),
            "a picture as large as anything served gives every width asked for"
        );
        assert_eq!(
            widths_worth_writing(&wanted, Some(1280)),
            vec![640, 1280],
            "and one held at a third of the largest gives what it really has, \
             not an enlargement the interface would then ask for"
        );
        assert_eq!(
            widths_worth_writing(&wanted, Some(1300)),
            vec![640, 1280, 1300],
            "the last of them follows the picture rather than being thrown away"
        );
        assert_eq!(
            widths_worth_writing(&wanted, Some(400)),
            vec![400],
            "a small picture is written once, at its own size"
        );
        assert_eq!(
            widths_worth_writing(&wanted, None),
            wanted.to_vec(),
            "a size nobody could read is not a reason to throw away detail"
        );
        assert_eq!(widths_worth_writing(&wanted, Some(0)), wanted.to_vec());
    }

    #[test]
    fn a_picture_prepared_by_an_older_recipe_is_not_the_one_wanted_now() {
        // What stands beside a picture says both what it was made from and how
        // it was made, so a rule written after a library was filled reaches it.
        let now = stamp("/one.jpg");
        assert!(now.starts_with(&format!("{RECIPE}-")));
        assert_ne!(now, stamp("/another.jpg"));
        assert_ne!(
            now,
            melyxar_core::fingerprint::of_text("/one.jpg"),
            "a picture stamped before there was a recipe is made again"
        );
    }

    #[test]
    fn a_height_follows_the_width_so_nothing_is_stretched() {
        assert_eq!(scaled_height(400, 1000, 1500), 600);
        assert_eq!(scaled_height(1280, 1920, 1080), 720);
    }

    #[test]
    fn a_picture_of_no_width_has_no_height_to_speak_of() {
        assert_eq!(scaled_height(400, 0, 1500), 0);
    }

    #[test]
    fn each_kind_of_picture_gets_the_widths_it_is_shown_at() {
        assert_eq!(Kind::Poster.widths()[0], 200);
        assert!(
            Kind::Backdrop.widths()[0] > Kind::Poster.widths()[0],
            "a backdrop is shown wide and a poster is not"
        );
        assert_eq!(Kind::Poster.as_str(), "poster");
        assert_eq!(Kind::Backdrop.as_str(), "backdrop");
        assert_eq!(Kind::Logo.as_str(), "logo");
    }

    #[test]
    fn a_title_image_belongs_to_its_film_and_paints_no_card() {
        assert_eq!(Kind::Logo.owner_kind(), "work");
        assert_eq!(Kind::Logo.folder(), Kind::Poster.folder());
        assert!(
            !Kind::Logo.carries_the_colour_of_its_work(),
            "a card is painted the colour of its poster; a title image is \
             mostly see-through and would paint it the colour of nothing"
        );
    }

    #[test]
    fn the_name_of_a_picture_follows_the_path_the_provider_gave() {
        let first = fingerprint::of_text("/abc123.jpg");
        assert_eq!(first, fingerprint::of_text("/abc123.jpg"));
        assert_ne!(
            first,
            fingerprint::of_text("/def456.jpg"),
            "a provider that changed the picture changed its path"
        );
    }
}
