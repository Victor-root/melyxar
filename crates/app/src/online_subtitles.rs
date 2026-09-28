//! Subtitles found and downloaded from OpenSubtitles, for an administrator,
//! with the key and the account they gave in the administration.
//!
//! A downloaded subtitle becomes one more track of the copy it was asked for,
//! kept in the server's own folder of downloaded subtitles rather than next to
//! the film, whose folders stay read only. A scan leaves it alone; only taking
//! it away by hand removes it.

use melyxar_core::id::{MediaSourceId, TrackId, WorkId};
use melyxar_core::media::{normalise_language, SubtitleDetails, SubtitleLayout, Track, TrackKind};
use melyxar_core::user::User;
use melyxar_core::work::WorkKind;
use melyxar_database::settings::OpenSubtitlesAccount;
use melyxar_metadata::opensubtitles::{OpenSubtitlesClient, Searched};
pub use melyxar_metadata::opensubtitles::SubtitleOffer;
use melyxar_metadata::ProviderError;

use crate::identify::known_id;
use crate::ratings::KeyTried;
use crate::{AppError, AppState, Result};

/// How old a downloaded file must be before a sweep may take it for one
/// nobody holds.
const SETTLED_AFTER: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Where OpenSubtitles stands on this server: whether a key was given, and an
/// account with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    pub has_key: bool,
    pub signed_in: bool,
}

/// A subtitle just downloaded, and how many more downloads today allows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub track_id: TrackId,
    pub remaining: Option<i64>,
}

/// Whether a key and an account were given.
pub async fn standing(state: &AppState) -> Result<Standing> {
    let account = state.database().opensubtitles_account().await?;
    Ok(Standing {
        has_key: account.is_some(),
        signed_in: account.is_some_and(|account| account.login.is_some()),
    })
}

/// Keeps the key and the account typed once OpenSubtitles has taken them.
/// An account is optional; a name without a password, or the other way
/// round, is none.
pub async fn set_account(
    state: &AppState,
    key: &str,
    username: &str,
    password: &str,
) -> Result<KeyTried> {
    let key = key.trim();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(KeyTried::Refused);
    }
    let login = match (username.trim(), password) {
        ("", _) | (_, "") => None,
        (username, password) => Some((username.to_string(), password.to_string())),
    };
    let Ok(client) = OpenSubtitlesClient::new(key, login.clone()) else {
        return Ok(KeyTried::Unreachable);
    };
    match client.check().await {
        Ok(()) | Err(ProviderError::TooManyRequests { .. }) => {}
        Err(ProviderError::Unauthorised) => return Ok(KeyTried::Refused),
        Err(error) => {
            tracing::warn!(%error, "OpenSubtitles could not be asked whether it knows the key");
            return Ok(KeyTried::Unreachable);
        }
    }
    state
        .database()
        .set_opensubtitles_account(Some(&OpenSubtitlesAccount {
            key: key.to_string(),
            login,
        }))
        .await?;
    tracing::info!("an OpenSubtitles key was kept");
    Ok(KeyTried::Kept)
}

/// Forgets the key and the account.
pub async fn forget_account(state: &AppState) -> Result<()> {
    state.database().set_opensubtitles_account(None).await?;
    tracing::info!("the OpenSubtitles key was taken away");
    Ok(())
}

/// The subtitles OpenSubtitles offers for the work of this copy, in these
/// languages, written with two letters.
pub async fn offers(
    state: &AppState,
    who: &User,
    source_id: MediaSourceId,
    languages: &[String],
) -> Result<Vec<SubtitleOffer>> {
    may_manage(who)?;
    if languages.is_empty() || !languages.iter().all(|language| is_a_language(language)) {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "languages are written with two letters",
        )));
    }
    let client = client(state).await?;
    let work_id = crate::playable_file(state.database(), source_id).await?.work_id;
    let searched = searched_for(state, work_id).await?;
    let asking = match &searched {
        Asked::Film(tmdb_id) => Searched::Film { tmdb_id },
        Asked::Episode(series, season, episode) => Searched::Episode {
            series_tmdb_id: series,
            season: *season,
            episode: *episode,
        },
    };
    client.search(asking, languages).await.map_err(said)
}

/// Downloads one subtitle offered and makes it a track of this copy.
pub async fn download(
    state: &AppState,
    who: &User,
    source_id: MediaSourceId,
    offer: &SubtitleOffer,
) -> Result<Fetched> {
    may_manage(who)?;
    let source = crate::playable_file(state.database(), source_id).await?;
    let client = client(state).await?;
    let fetched = client.download(offer.file_id).await.map_err(said)?;

    let track_id = TrackId::new();
    let file = format!("{track_id}.srt");
    let folder = state.config().directories.downloaded_subtitles();
    tokio::fs::create_dir_all(&folder).await.map_err(AppError::Directory)?;
    tokio::fs::write(folder.join(&file), &fetched.contents)
        .await
        .map_err(AppError::Directory)?;

    let track = Track {
        id: track_id,
        source_id: source.id,
        stream_index: 0,
        language: Some(normalise_language(&offer.language)),
        title: Some(offer.release.clone()).filter(|release| !release.is_empty()),
        is_default: false,
        is_forced: false,
        kind: TrackKind::Subtitle(SubtitleDetails {
            codec: "subrip".to_string(),
            layout: SubtitleLayout::Text,
            is_hearing_impaired: offer.hearing_impaired,
            is_external: true,
            external_relative_path: None,
            downloaded_file: Some(file),
        }),
    };
    state.database().add_downloaded_subtitle(source.id, &track).await?;
    tracing::info!(
        release = %offer.release,
        language = %offer.language,
        remaining = fetched.remaining,
        "a subtitle was downloaded"
    );
    Ok(Fetched {
        track_id,
        remaining: fetched.remaining,
    })
}

/// The subtitle tracks of this copy, for the window that adds and takes them
/// away: inside the film, beside it, or downloaded.
pub async fn tracks_of(state: &AppState, who: &User, source_id: MediaSourceId) -> Result<Vec<Track>> {
    may_manage(who)?;
    crate::reach::may_read_the_copy(state, who, source_id).await?;
    Ok(state
        .database()
        .tracks_of_source(source_id)
        .await?
        .into_iter()
        .filter(|track| matches!(track.kind, TrackKind::Subtitle(_)))
        .collect())
}

/// Takes away a subtitle downloaded for this copy, its file with it.
pub async fn remove(state: &AppState, who: &User, source_id: MediaSourceId, track_id: TrackId) -> Result<()> {
    may_manage(who)?;
    let file = state
        .database()
        .remove_downloaded_subtitle(source_id, track_id)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("downloaded subtitle")))?;
    let directories = &state.config().directories;
    let _ = tokio::fs::remove_file(directories.downloaded_subtitles().join(file)).await;
    let _ = tokio::fs::remove_file(crate::subtitles::cached_at(state, track_id)).await;
    Ok(())
}

/// Deletes the files of downloaded subtitles no track holds any more: the
/// copy they were downloaded for was deleted, or left the disk and the
/// library with it. Answers how many went.
pub async fn forget_the_orphans(state: &AppState) -> usize {
    let held = match state.database().downloaded_subtitle_files().await {
        Ok(held) => held,
        Err(error) => {
            tracing::warn!(%error, "the downloaded subtitles still held could not be read");
            return 0;
        }
    };
    let folder = state.config().directories.downloaded_subtitles();
    let Ok(mut entries) = tokio::fs::read_dir(&folder).await else {
        return 0;
    };
    let mut on_the_disk = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        // A file is written before its track, so one this recent may be a
        // download still under way rather than one nobody holds.
        let settled = entry
            .metadata()
            .await
            .ok()
            .and_then(|found| found.modified().ok())
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age >= SETTLED_AFTER);
        if let (true, Some(name)) = (settled, entry.file_name().to_str()) {
            on_the_disk.push(name.to_string());
        }
    }
    let mut gone = 0;
    for name in orphans(&on_the_disk, &held) {
        if tokio::fs::remove_file(folder.join(name)).await.is_ok() {
            gone += 1;
        }
    }
    if gone > 0 {
        tracing::info!(gone, "downloaded subtitles no copy holds any more were deleted");
    }
    gone
}

/// The files of the folder no track holds.
fn orphans<'a>(on_the_disk: &'a [String], held: &[String]) -> Vec<&'a str> {
    on_the_disk
        .iter()
        .filter(|name| !held.contains(name))
        .map(String::as_str)
        .collect()
}

/// What OpenSubtitles is asked about for a work.
enum Asked {
    Film(String),
    Episode(String, i32, i32),
}

/// A film by what TMDb calls it; an episode by what TMDb calls its series,
/// with the number of its season and its own.
async fn searched_for(state: &AppState, work_id: WorkId) -> Result<Asked> {
    let database = state.database();
    let work = database
        .work(work_id)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("work")))?;
    let unknown = || {
        AppError::Domain(melyxar_core::Error::not_described(
            "this work is not named by the provider, so its subtitles cannot be looked for",
        ))
    };
    match work.kind {
        WorkKind::Movie => known_id(&database.work_external_ids(work.id).await?, "tmdb")
            .map(Asked::Film)
            .ok_or_else(unknown),
        WorkKind::Episode => {
            let season = match work.parent_id {
                Some(id) => database.work(id).await?,
                None => None,
            }
            .ok_or_else(unknown)?;
            let series = season.parent_id.ok_or_else(unknown)?;
            let series_id = known_id(&database.work_external_ids(series).await?, "tmdb").ok_or_else(unknown)?;
            match (season.ordinal, work.ordinal) {
                (Some(season), Some(episode)) => Ok(Asked::Episode(series_id, season, episode)),
                _ => Err(unknown()),
            }
        }
        _ => Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "only a film or an episode has subtitles to look for",
        ))),
    }
}

/// The client OpenSubtitles is asked through, or why there is none.
async fn client(state: &AppState) -> Result<OpenSubtitlesClient> {
    let account = state.database().opensubtitles_account().await?.ok_or_else(|| {
        AppError::Domain(melyxar_core::Error::dependency_missing(
            "no OpenSubtitles key was given",
        ))
    })?;
    OpenSubtitlesClient::new(account.key, account.login).map_err(said)
}

/// What went wrong with OpenSubtitles, in the words a screen can say.
fn said(error: ProviderError) -> AppError {
    tracing::warn!(%error, "OpenSubtitles would not answer");
    let code = match error {
        ProviderError::Unauthorised => melyxar_core::error::ErrorCode::Forbidden,
        ProviderError::TooManyRequests { .. } => melyxar_core::error::ErrorCode::TooManyAttempts,
        _ => melyxar_core::error::ErrorCode::ExternalServiceUnavailable,
    };
    AppError::Domain(melyxar_core::Error::new(code, error.to_string()))
}

fn may_manage(who: &User) -> Result<()> {
    match who.permissions.is_administrator {
        true => Ok(()),
        false => Err(AppError::Domain(melyxar_core::Error::forbidden(
            "only an administrator looks for subtitles",
        ))),
    }
}

/// A language as OpenSubtitles writes it: two letters, and a region after a
/// dash for the ones it tells apart.
fn is_a_language(value: &str) -> bool {
    let mut parts = value.split('-');
    let two = |part: Option<&str>| part.is_some_and(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_lowercase()));
    two(parts.next()) && parts.next().is_none_or(|region| two(Some(region))) && parts.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_files_no_track_holds_are_orphans() {
        let on_the_disk = ["a.srt".to_string(), "b.srt".to_string()];
        assert_eq!(orphans(&on_the_disk, &["b.srt".to_string()]), vec!["a.srt"]);
        assert!(orphans(&on_the_disk, &on_the_disk).is_empty());
    }

    #[test]
    fn a_language_is_two_letters_with_a_region_at_most() {
        assert!(is_a_language("fr"));
        assert!(is_a_language("pt-br"));
        assert!(!is_a_language("fre"));
        assert!(!is_a_language("FR"));
        assert!(!is_a_language("pt-br-x"));
        assert!(!is_a_language(""));
    }
}
