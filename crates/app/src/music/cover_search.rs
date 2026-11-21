//! The covers of an album searched by hand from the tag manager, on
//! MusicBrainz and Deezer at once, for somebody to choose one by eye.
//!
//! Only for an account that may write into the album, in a library that lets
//! its tag manager search online. The pictures are fetched by the server and
//! handed on as they come: the browser never reaches either service, and
//! nothing is kept.

use melyxar_core::id::WorkId;
use melyxar_core::user::User;
use melyxar_metadata::ProviderError;
pub use melyxar_metadata::album_covers::{CoverSize, CoverSource, OfferedCover};
use melyxar_metadata::deezer::DeezerClient;
use melyxar_metadata::musicbrainz::MusicBrainzClient;

use crate::music::tag_editing::writable_album;
use crate::{AppError, AppState, Result};

/// What a search found, and the services that did not answer it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    pub covers: Vec<OfferedCover>,
    pub unanswered: Vec<CoverSource>,
}

/// The covers both services hold for an album title and its artist,
/// MusicBrainz's first. A service that does not answer leaves only the
/// other's, and says so.
pub async fn offered(
    state: &AppState,
    who: &User,
    album: WorkId,
    title: &str,
    artist: Option<&str>,
) -> Result<Offered> {
    may_search(state, who, album).await?;
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "an album title is needed to search for its cover",
        )));
    }
    let artist = artist.map(str::trim).filter(|artist| !artist.is_empty());
    let (musicbrainz, deezer) = tokio::join!(
        asked_of_musicbrainz(title, artist),
        async { DeezerClient::new()?.albums(title, artist).await },
    );
    Ok(gathered([
        (CoverSource::MusicBrainz, musicbrainz),
        (CoverSource::Deezer, deezer),
    ]))
}

/// The albums MusicBrainz has for a title and an artist, asked a second time
/// after the pause it asks for when it says it is busy: it says so of any
/// second question within a second of the first, which a search following
/// another or the covers looked up in the background is often enough.
async fn asked_of_musicbrainz(
    title: &str,
    artist: Option<&str>,
) -> std::result::Result<Vec<OfferedCover>, ProviderError> {
    let client = MusicBrainzClient::new()?;
    match client.albums(title, artist).await {
        Err(ProviderError::TooManyRequests { .. }) => {
            tokio::time::sleep(melyxar_metadata::musicbrainz::BETWEEN_QUESTIONS).await;
            client.albums(title, artist).await
        }
        answered => answered,
    }
}

/// One offered cover, at the size asked, as a JPEG.
pub async fn picture(
    state: &AppState,
    who: &User,
    album: WorkId,
    source: CoverSource,
    key: &str,
    size: CoverSize,
) -> Result<Vec<u8>> {
    may_search(state, who, album).await?;
    if !source.takes(key) {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "this is not a cover of that service",
        )));
    }
    let fetched = match source {
        CoverSource::MusicBrainz => MusicBrainzClient::new()
            .map_err(said)?
            .cover(key, size)
            .await
            .map_err(said)?,
        CoverSource::Deezer => Some(
            DeezerClient::new()
                .map_err(said)?
                .cover(key, size)
                .await
                .map_err(said)?,
        ),
    };
    fetched.ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("cover")))
}

/// Every cover found, in the order the services are given, and those that
/// did not answer.
fn gathered<const N: usize>(
    answers: [(CoverSource, std::result::Result<Vec<OfferedCover>, ProviderError>); N],
) -> Offered {
    let mut offered = Offered {
        covers: Vec::new(),
        unanswered: Vec::new(),
    };
    for (source, answer) in answers {
        match answer {
            Ok(covers) => offered.covers.extend(covers),
            Err(error) => {
                tracing::warn!(service = source.as_str(), %error, "a search for covers went unanswered");
                offered.unanswered.push(source);
            }
        }
    }
    offered
}

/// Whether this account may search covers for this album: it may write into
/// the album, and its library lets the tag manager search online.
async fn may_search(state: &AppState, who: &User, album: WorkId) -> Result<()> {
    let on_disk = writable_album(state, who, album).await?;
    match state
        .database()
        .music_library_options(on_disk.library_id)
        .await?
        .cover_search_online
    {
        true => Ok(()),
        false => Err(AppError::Domain(melyxar_core::Error::forbidden(
            "this library does not search covers online",
        ))),
    }
}

/// What went wrong with a service, in the words a screen can say.
fn said(error: ProviderError) -> AppError {
    tracing::warn!(%error, "a cover could not be fetched");
    let code = match error {
        ProviderError::TooManyRequests { .. } => melyxar_core::error::ErrorCode::TooManyAttempts,
        _ => melyxar_core::error::ErrorCode::ExternalServiceUnavailable,
    };
    AppError::Domain(melyxar_core::Error::new(code, error.to_string()))
}

#[cfg(test)]
mod tests {
    use melyxar_core::music_preferences::MusicLibraryOptions;

    use super::*;
    use crate::music::testing::{album_of, music_library, put, scan, song_of};

    fn refused_with(outcome: Result<impl std::fmt::Debug>, words: &str) -> bool {
        matches!(outcome, Err(AppError::Domain(error)) if error.to_string().contains(words))
    }

    #[tokio::test]
    async fn covers_are_searched_only_where_the_library_lets_its_tag_manager_search() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("Music");
        let relative = "Amber Field/Northern Lights/01 - Quiet Harbour.flac";
        put(&root, relative, "one-second.flac");
        let (state, library) = music_library(directory.path(), &root).await;
        scan(&state, &library).await;
        let album = album_of(&state, &song_of(&state, &library, relative).await).await.id;
        let administrator = state
            .database()
            .create_user("Boss", None, &melyxar_core::user::Permissions::administrator())
            .await
            .expect("account created");
        let options = |cover_search_online| MusicLibraryOptions {
            tag_writing: true,
            cover_search_online,
            ..Default::default()
        };

        state
            .database()
            .set_music_library_options(library.id, &options(false))
            .await
            .expect("set");
        assert!(refused_with(
            offered(&state, &administrator, album, "Northern Lights", None).await,
            "does not search covers online"
        ));
        assert!(refused_with(
            picture(&state, &administrator, album, CoverSource::Deezer, "5718f7c81c27e0b2417e2a4c45224f8a", CoverSize::Small).await,
            "does not search covers online"
        ));

        state
            .database()
            .set_music_library_options(library.id, &options(true))
            .await
            .expect("set");
        assert!(refused_with(
            offered(&state, &administrator, album, "  ", None).await,
            "an album title is needed"
        ));
        assert!(
            refused_with(
                picture(&state, &administrator, album, CoverSource::Deezer, "../../etc/passwd", CoverSize::Large).await,
                "not a cover of that service"
            ),
            "nothing but a key of the service is ever fetched"
        );

        let viewer = crate::a_viewer(&state).await;
        assert!(refused_with(
            offered(&state, &viewer, album, "Northern Lights", None).await,
            "may not edit the tags"
        ));
    }

    fn cover(source: CoverSource, key: &str) -> OfferedCover {
        OfferedCover {
            source,
            key: key.to_string(),
            title: "Northern Lights".to_string(),
            artist: None,
            year: None,
        }
    }

    #[test]
    fn a_service_that_does_not_answer_leaves_the_other_s_covers_and_is_named() {
        let offered = gathered([
            (
                CoverSource::MusicBrainz,
                Err(ProviderError::TooManyRequests {
                    retry_after_seconds: None,
                }),
            ),
            (CoverSource::Deezer, Ok(vec![cover(CoverSource::Deezer, "a")])),
        ]);
        assert_eq!(offered.covers, vec![cover(CoverSource::Deezer, "a")]);
        assert_eq!(offered.unanswered, vec![CoverSource::MusicBrainz]);
    }

    #[test]
    fn the_covers_come_in_the_order_of_the_services() {
        let offered = gathered([
            (CoverSource::MusicBrainz, Ok(vec![cover(CoverSource::MusicBrainz, "m")])),
            (CoverSource::Deezer, Ok(vec![cover(CoverSource::Deezer, "d")])),
        ]);
        assert_eq!(
            offered.covers,
            vec![cover(CoverSource::MusicBrainz, "m"), cover(CoverSource::Deezer, "d")]
        );
        assert!(offered.unanswered.is_empty());
    }
}
