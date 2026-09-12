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

use std::path::Path;

use melyxar_core::fingerprint;
use melyxar_core::id::WorkId;
use melyxar_database::images::StoredImage;
use melyxar_metadata::{MetadataProvider, MovieDetails};

use crate::{AppState, Result};

/// What a picture is for, which decides the widths generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Poster,
    Backdrop,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Poster => "poster",
            Self::Backdrop => "backdrop",
        }
    }

    fn widths(self) -> &'static [u32] {
        match self {
            Self::Poster => &melyxar_ffmpeg::images::POSTER_WIDTHS,
            Self::Backdrop => &melyxar_ffmpeg::images::BACKDROP_WIDTHS,
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
    details: &MovieDetails,
) -> usize {
    let Some(tools) = state.tools() else {
        return 0;
    };

    let mut prepared = 0;
    for (kind, path) in [
        (Kind::Poster, details.poster_path.as_deref()),
        (Kind::Backdrop, details.backdrop_path.as_deref()),
    ] {
        let Some(path) = path else { continue };
        match store_one(state, provider, &tools.ffmpeg, work_id, kind, path).await {
            Ok(true) => prepared += 1,
            Ok(false) => {}
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

/// Answers whether anything was actually prepared.
async fn store_one(
    state: &AppState,
    provider: &impl MetadataProvider,
    tool: &Path,
    work_id: WorkId,
    kind: Kind,
    provider_path: &str,
) -> Result<bool> {
    let database = state.database();
    let owner_id = work_id.to_db_string();
    // The provider changes the path when it changes the picture, so the path
    // already says whether anything is new. Hashing the bytes would mean
    // fetching them first, which is the very thing to avoid.
    let fingerprint = fingerprint::of_text(provider_path);

    if database
        .image_fingerprint("work", &owner_id, kind.as_str())
        .await?
        .as_deref()
        == Some(fingerprint.as_str())
    {
        return Ok(false);
    }

    let bytes = match provider.fetch_image(provider_path).await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(
                kind = kind.as_str(),
                reason = %error,
                "the picture could not be fetched; it will be asked for again later"
            );
            return Ok(false);
        }
    };

    let root = state.config().directories.images();
    let folder = root.join("works").join(&owner_id);
    tokio::fs::create_dir_all(&folder).await?;

    // The picture as it arrived is kept only while the sizes are made from it.
    let original = folder.join(format!("{}-{fingerprint}.source", kind.as_str()));
    tokio::fs::write(&original, &bytes).await?;

    let colour = melyxar_ffmpeg::images::average_colour(tool, &original)
        .await
        .ok();
    let source_size = source_dimensions(state, &original).await;

    let mut prepared = Vec::new();
    for width in kind.widths() {
        let name = format!("{}-{fingerprint}-{width}.webp", kind.as_str());
        let destination = folder.join(&name);
        if let Err(error) =
            melyxar_ffmpeg::images::resize(tool, &original, &destination, *width).await
        {
            tracing::warn!(
                kind = kind.as_str(),
                width = width,
                error = %error,
                "one size of a picture could not be written"
            );
            continue;
        }
        prepared.push(StoredImage {
            owner_kind: "work".to_string(),
            owner_id: owner_id.clone(),
            image_kind: kind.as_str().to_string(),
            relative_path: format!("works/{owner_id}/{name}"),
            width: Some(*width as i32),
            height: source_size.map(|(w, h)| scaled_height(*width, w, h)),
            fingerprint: fingerprint.clone(),
            dominant_color: colour.clone(),
        });
    }

    // The picture as it arrived has done its work.
    tokio::fs::remove_file(&original).await.ok();

    if prepared.is_empty() {
        return Ok(false);
    }

    let no_longer_used = database
        .replace_images("work", &owner_id, kind.as_str(), &prepared)
        .await?;
    for path in no_longer_used {
        tokio::fs::remove_file(root.join(path)).await.ok();
    }

    // The card shows a colour before any picture arrives, and the poster is
    // what a card shows, so it is the poster that gives the colour.
    if kind == Kind::Poster {
        if let Some(colour) = colour {
            database.set_work_dominant_color(work_id, &colour).await?;
        }
    }
    Ok(true)
}

/// The size of the picture as it arrived, so a client can leave the right
/// space for it before it loads.
async fn source_dimensions(state: &AppState, path: &Path) -> Option<(i32, i32)> {
    let tools = state.tools()?;
    let report = melyxar_ffmpeg::probe::probe(&tools.ffprobe, path)
        .await
        .ok()?;
    let stream = report.streams.first()?;
    match (stream.width, stream.height) {
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
