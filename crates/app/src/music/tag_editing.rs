//! The tag manager: what the songs of an album carry in their files, what
//! would change, and writing it, renaming each file by a pattern where one
//! is given (see `docs/architecture/06-musique.md`).
//!
//! Only an account given the right does it, and only in a library that lets
//! its files be written. A copy of each file can be kept before it is
//! written. The files are renamed where they lie, never moved, and the scan
//! that follows reads them again under the same songs, so nothing kept about
//! them is lost.
//!
//! A song can also be cut short at either end, the silence a ripper left
//! before the first note for one: the file itself is cut, its sound carried
//! over without being encoded again and its tags put back as they were.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::job::JobPriority;
use melyxar_core::music_naming::{NameValues, name_from_pattern};
use melyxar_core::refresh::RefreshMode;
use melyxar_core::time::Millis;
use melyxar_core::user::User;
use melyxar_database::music_tag_editing::SongOnDisk;
use melyxar_ffmpeg::AskedToStop;
use melyxar_ffmpeg::song_analysis::{self, analyse};
use melyxar_sound::{WAVE_MOMENTS_A_SECOND, WaveReading};
pub use melyxar_tags::EditedTags;

use crate::reach::may_read;
use crate::{AppError, AppState, Result};

/// What one song's file carries now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongTags {
    pub song: WorkId,
    pub file_name: String,
    pub tags: EditedTags,
    /// How long it plays, when the scan could tell: what a cut is made in.
    pub duration: Option<Millis>,
}

/// What a person wants a song's file to carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    pub song: WorkId,
    pub tags: EditedTags,
}

/// What writing would do to one song.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub song: WorkId,
    pub file_name: String,
    /// The name it would be given, when the pattern changes it.
    pub new_file_name: Option<String>,
    /// The fields that would change, by name.
    pub changed: Vec<&'static str>,
    pub before: EditedTags,
    pub after: EditedTags,
}

/// What came of writing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Written {
    pub written: usize,
    /// The songs that could not be written, and why.
    pub failed: Vec<(WorkId, String)>,
}

/// How loud a song is along its length, read from its file as it is now, to
/// cut it on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wave {
    pub moments_a_second: u32,
    /// A byte a moment (see `melyxar_sound::WaveReading`).
    pub levels: Vec<u8>,
}

/// The shortest a song may be cut to: anything shorter is a slip of the hand
/// rather than a song.
const SHORTEST_CUT: Millis = Millis::new(1_000);

/// Where the copies of the files are kept before they are written.
const BACKUPS: &str = "tag-backups";

/// How long a copy is kept: long enough to notice a change gone wrong and
/// put it back, short of filling the disk with every song ever retagged.
const KEPT_FOR: time::Duration = time::Duration::days(30);

/// The folder the copies made now go in, each folder named by the moment it
/// was made. The copies past keeping are cleared away first.
async fn backups_now(state: &AppState) -> PathBuf {
    let all = state.config().directories.data.join(BACKUPS);
    let now = melyxar_core::time::now().unix_timestamp();
    if let Ok(mut kept) = tokio::fs::read_dir(&all).await {
        while let Ok(Some(entry)) = kept.next_entry().await {
            if entry.file_name().to_str().is_some_and(|made| past_keeping(made, now))
                && let Err(error) = tokio::fs::remove_dir_all(entry.path()).await
            {
                tracing::warn!(folder = %entry.path().display(), %error, "an old copy of songs could not be removed");
            }
        }
    }
    all.join(now.to_string())
}

/// Whether the copies made at this moment, as their folder is named, are
/// past keeping. A folder named otherwise is not one of them, and stays.
fn past_keeping(made: &str, now: i64) -> bool {
    made.parse::<i64>()
        .is_ok_and(|made| now.saturating_sub(made) > KEPT_FOR.whole_seconds())
}

/// What the songs of an album carry in their files, read from the files.
pub async fn album_tags(state: &AppState, who: &User, album: WorkId) -> Result<Vec<SongTags>> {
    let mut found = Vec::new();
    for song in state.database().songs_of_album(album).await? {
        let on_disk = writable(state, who, song).await?;
        found.push(SongTags {
            song,
            file_name: file_name_of(&on_disk.relative_path),
            tags: read_tags(on_disk.path()).await?,
            duration: on_disk.duration,
        });
    }
    Ok(found)
}

/// What writing would do, without writing anything.
pub async fn plan(
    state: &AppState,
    who: &User,
    wanted: &[Wanted],
    pattern: Option<&str>,
) -> Result<Vec<Planned>> {
    let mut planned = Vec::with_capacity(wanted.len());
    for one in wanted {
        let on_disk = writable(state, who, one.song).await?;
        let before = read_tags(on_disk.path()).await?;
        let file_name = file_name_of(&on_disk.relative_path);
        let new_file_name = match pattern.map(str::trim).filter(|pattern| !pattern.is_empty()) {
            Some(pattern) => Some(renamed(pattern, &one.tags, &on_disk.relative_path)?),
            None => None,
        }
        .filter(|name| *name != file_name);
        planned.push(Planned {
            song: one.song,
            changed: one.tags.changed_from(&before),
            file_name,
            new_file_name,
            before,
            after: one.tags.clone(),
        });
    }
    Ok(planned)
}

/// Writes what is wanted into the files, renaming them by the pattern when
/// one is given, and keeping a copy of each first when asked. The library is
/// scanned again once it is done, which is what files the songs anew.
pub async fn write(
    state: &AppState,
    who: &User,
    wanted: &[Wanted],
    pattern: Option<&str>,
    keep_a_copy: bool,
) -> Result<Written> {
    let planned = plan(state, who, wanted, pattern).await?;
    let backups = backups_now(state).await;
    let mut written = Written::default();
    let mut libraries: Vec<LibraryId> = Vec::new();
    // A song that nothing changes is left alone, and so is its file: no copy
    // of it is kept and the library is not read again for it.
    for plan in planned
        .into_iter()
        .filter(|plan| !plan.changed.is_empty() || plan.new_file_name.is_some())
    {
        let Some(on_disk) = state.database().song_on_disk(plan.song).await? else {
            continue;
        };
        if !libraries.contains(&on_disk.library_id) {
            libraries.push(on_disk.library_id);
        }
        match write_one(
            state,
            &on_disk,
            &plan,
            keep_a_copy.then_some(backups.as_path()),
        )
        .await
        {
            Ok(()) => written.written += 1,
            Err(reason) => {
                tracing::warn!(file = %on_disk.path().display(), reason, "a song's tags could not be written");
                written.failed.push((plan.song, reason));
            }
        }
    }
    for library in libraries {
        read_the_library_again(state, library).await;
    }
    Ok(written)
}

/// Cuts a song's file down to what lies between `from` and `to`, keeping a
/// copy of it first when asked. The cut is written beside the file and put in
/// its place only once it is whole, so a failure leaves the song as it was.
pub async fn trim(
    state: &AppState,
    who: &User,
    song: WorkId,
    from: Millis,
    to: Millis,
    keep_a_copy: bool,
) -> Result<()> {
    let on_disk = writable(state, who, song).await?;
    let tools = state
        .tools()
        .cloned()
        .ok_or_else(|| refused("the media tools are not installed"))?;
    if !cut_holds(from, to, on_disk.duration) {
        return Err(refused(
            "a song is cut between its start and its end, and kept at least a second long",
        ));
    }
    let path = on_disk.path();
    if keep_a_copy {
        let backup = backups_now(state).await.join(&on_disk.relative_path);
        copy_into(&path, &backup).await.map_err(refused)?;
    }
    let cut = cut_beside(&path);
    let made = async {
        melyxar_ffmpeg::song_cutting::cut(&tools, &path, from, to, &cut)
            .await
            .map_err(|error| error.to_string())?;
        let (tagged, untagged) = (path.clone(), cut.clone());
        tokio::task::spawn_blocking(move || melyxar_tags::copy_tags(&tagged, &untagged))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        tokio::fs::rename(&cut, &path)
            .await
            .map_err(|error| format!("the cut could not be put in place: {error}"))
    }
    .await;
    if let Err(reason) = made {
        if let Err(error) = tokio::fs::remove_file(&cut).await
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(file = %cut.display(), %error, "a cut left halfway could not be removed");
        }
        tracing::warn!(file = %path.display(), reason, "a song could not be cut");
        return Err(refused(reason));
    }
    read_the_library_again(state, on_disk.library_id).await;
    Ok(())
}

/// The wave of a song, to cut it or to place the lines of its lyrics on it,
/// read from its file every time rather than kept: a cut changes the file,
/// and the spectrum kept from the scan follows only once the song has been
/// analysed again. Reading it writes nothing, so it is for whoever edits the
/// tags of songs or their lyrics, whether or not the library lets its files
/// be written.
pub async fn wave(state: &AppState, who: &User, song: WorkId) -> Result<Wave> {
    if !who.permissions.may_edit_tags && !who.permissions.is_administrator {
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::Forbidden,
            "this account may neither edit the tags of songs nor their lyrics",
        )));
    }
    let on_disk = on_disk(state, who, song).await?;
    let tools = state
        .tools()
        .cloned()
        .ok_or_else(|| refused("the media tools are not installed"))?;
    let reading = Arc::new(Mutex::new(WaveReading::default()));
    let hearing = Arc::clone(&reading);
    let wanted = song_analysis::Wanted {
        loudness: false,
        spectrum: true,
    };
    // Heard on the thread the sound is read on, never one that serves
    // requests. A page closed meanwhile drops this, and the tool with it.
    analyse(
        &tools,
        &on_disk.path(),
        wanted,
        AskedToStop::never(),
        move |samples| {
            hearing
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .hear(samples);
        },
    )
    .await
    .map_err(|error| refused(error.to_string()))?;
    let reading = std::mem::take(&mut *reading.lock().unwrap_or_else(PoisonError::into_inner));
    Ok(Wave {
        moments_a_second: WAVE_MOMENTS_A_SECOND,
        levels: reading.levels(),
    })
}

/// Whether a cut from `from` to `to` keeps a song worth the name, within the
/// length the scan found when it found one.
fn cut_holds(from: Millis, to: Millis, length: Option<Millis>) -> bool {
    from.get() >= 0
        && to.get() - from.get() >= SHORTEST_CUT.get()
        && length.is_none_or(|length| to <= length)
}

/// Where the cut of a file is written while it is made: beside it, hidden so
/// that a scan running meanwhile does not take it for a song, and named with
/// the same extension, which says what it is written as.
fn cut_beside(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".melyxar-cut-{name}"))
}

/// Puts a picture beside the songs of an album as its cover, `cover.jpg` in
/// the album's folder, a disc's folder never. The picture is a JPEG, which
/// the page that sends it makes it.
pub async fn set_cover(
    state: &AppState,
    who: &User,
    album: WorkId,
    jpeg: Vec<u8>,
    keep_a_copy: bool,
) -> Result<()> {
    if !jpeg.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Err(refused("a cover is sent as a JPEG picture"));
    }
    let songs = state.database().songs_of_album(album).await?;
    let Some(first) = songs.first() else {
        return Err(AppError::Domain(melyxar_core::Error::not_found("album")));
    };
    let on_disk = writable(state, who, *first).await?;
    let path = on_disk.path();
    let folder = path
        .parent()
        .map(melyxar_library::music::album_folder)
        .ok_or_else(|| refused("the album has no folder"))?
        .to_path_buf();
    let cover = folder.join("cover.jpg");
    if keep_a_copy && tokio::fs::try_exists(&cover).await.unwrap_or(false) {
        let relative = cover
            .strip_prefix(&on_disk.root_path)
            .unwrap_or(&cover)
            .to_path_buf();
        let backup = backups_now(state).await.join(relative);
        copy_into(&cover, &backup).await.map_err(refused)?;
    }
    tokio::fs::write(&cover, jpeg)
        .await
        .map_err(|error| refused(format!("the cover could not be written: {error}")))?;
    read_the_library_again(state, on_disk.library_id).await;
    Ok(())
}

async fn write_one(
    state: &AppState,
    on_disk: &SongOnDisk,
    plan: &Planned,
    backups: Option<&Path>,
) -> std::result::Result<(), String> {
    let path = on_disk.path();
    if let Some(backups) = backups {
        copy_into(&path, &backups.join(&on_disk.relative_path)).await?;
    }
    if !plan.changed.is_empty() {
        let tags = plan.after.clone();
        let target = path.clone();
        tokio::task::spawn_blocking(move || melyxar_tags::write(&target, &tags))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
    }
    if let Some(name) = &plan.new_file_name {
        let renamed = on_disk.relative_path.with_file_name(name);
        let to = on_disk.root_path.join(&renamed);
        if tokio::fs::try_exists(&to).await.unwrap_or(false) {
            return Err(format!("a file named {name} is already there"));
        }
        tokio::fs::rename(&path, &to)
            .await
            .map_err(|error| format!("the file could not be renamed: {error}"))?;
        state
            .database()
            .rename_source_file(on_disk.source_id, &renamed)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// A copy of a file, into its folder of copies.
async fn copy_into(from: &Path, to: &Path) -> std::result::Result<(), String> {
    if let Some(folder) = to.parent() {
        tokio::fs::create_dir_all(folder)
            .await
            .map_err(|error| format!("no copy could be kept: {error}"))?;
    }
    tokio::fs::copy(from, to)
        .await
        .map(|_| ())
        .map_err(|error| format!("no copy could be kept: {error}"))
}

/// The song's file, once it is sure this account may read it.
async fn on_disk(state: &AppState, who: &User, song: WorkId) -> Result<SongOnDisk> {
    let on_disk = state
        .database()
        .song_on_disk(song)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("song")))?;
    may_read(who, on_disk.library_id)?;
    Ok(on_disk)
}

/// The song's file, once it is sure this account may write into it.
async fn writable(state: &AppState, who: &User, song: WorkId) -> Result<SongOnDisk> {
    if !who.permissions.may_edit_tags {
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::Forbidden,
            "this account may not edit the tags of songs",
        )));
    }
    let on_disk = on_disk(state, who, song).await?;
    if !state
        .database()
        .music_library_options(on_disk.library_id)
        .await?
        .tag_writing
    {
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::Forbidden,
            "this library does not let its files be written",
        )));
    }
    Ok(on_disk)
}

async fn read_tags(path: PathBuf) -> Result<EditedTags> {
    let read = tokio::task::spawn_blocking(move || melyxar_tags::read(&path))
        .await
        .map_err(|error| refused(error.to_string()))?
        .map_err(|error| refused(error.to_string()))?;
    Ok(EditedTags::from(&read.tags))
}

/// The name a pattern gives a file, keeping its extension.
fn renamed(pattern: &str, tags: &EditedTags, relative_path: &Path) -> Result<String> {
    let extension = relative_path
        .extension()
        .map(|extension| extension.to_string_lossy().into_owned())
        .unwrap_or_default();
    name_from_pattern(pattern, &name_values(tags), &extension)
        .ok_or_else(|| refused("the pattern names something no song has, or gives an empty name"))
}

/// What a pattern can name a file by, from the tags it is being given.
fn name_values(tags: &EditedTags) -> NameValues {
    NameValues {
        title: tags.title.clone(),
        artist: (!tags.artists.is_empty()).then(|| tags.artists.join(", ")),
        album: tags.album.clone(),
        album_artist: (!tags.album_artists.is_empty()).then(|| tags.album_artists.join(", ")),
        track: tags.track,
        disc: tags.disc,
        year: tags.year,
    }
}

fn file_name_of(relative_path: &Path) -> String {
    relative_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A scan of the library, which reads the written files again under the
/// same songs.
async fn read_the_library_again(state: &AppState, library: LibraryId) {
    let started = match crate::identify::library_of(state, library).await {
        Ok(library) => crate::scan::start_scan_and_identification(
            state,
            library,
            JobPriority::REQUESTED,
            RefreshMode::NewAndUpdatedFiles,
        )
        .await
        .map(|_| ()),
        Err(error) => Err(error),
    };
    if let Err(error) = started {
        tracing::warn!(%error, "the library could not be read again after its tags were written");
    }
}

fn refused(reason: impl Into<String>) -> AppError {
    AppError::Domain(melyxar_core::Error::invalid_input(reason))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cut_keeps_a_second_at_least_and_stays_within_the_song() {
        let length = Some(Millis::new(200_000));
        assert!(cut_holds(Millis::new(2_400), Millis::new(200_000), length));
        assert!(!cut_holds(Millis::new(-1), Millis::new(10_000), length), "before the start");
        assert!(!cut_holds(Millis::new(5_000), Millis::new(5_900), length), "under a second");
        assert!(!cut_holds(Millis::new(0), Millis::new(200_001), length), "past the end");
        assert!(cut_holds(Millis::new(0), Millis::new(900_000), None), "a length nobody knows");
    }

    #[tokio::test]
    async fn the_wave_is_for_an_administrator_even_where_files_are_not_written() {
        let (_directory, state) = crate::an_empty_server().await;
        let viewer = crate::a_viewer(&state).await;
        let forbidden = |outcome: Result<Wave>| {
            matches!(outcome, Err(AppError::Domain(error)) if error.to_string().contains("may neither edit"))
        };
        assert!(forbidden(wave(&state, &viewer, WorkId::new()).await));

        let administrator = state
            .database()
            .create_user("Boss", None, &melyxar_core::user::Permissions::administrator())
            .await
            .expect("account created");
        let missing = wave(&state, &administrator, WorkId::new()).await;
        assert!(
            matches!(missing, Err(AppError::Domain(error)) if error.to_string().contains("song")),
            "let through, to a song that is not there"
        );
    }

    #[test]
    fn a_cut_is_made_hidden_beside_its_song_under_the_same_extension() {
        assert_eq!(
            cut_beside(Path::new("/mnt/one/Quiet Harbour/01 - Low Tide.flac")),
            PathBuf::from("/mnt/one/Quiet Harbour/.melyxar-cut-01 - Low Tide.flac")
        );
    }

    #[test]
    fn a_copy_is_kept_a_month_and_what_is_not_a_copy_stays() {
        let now = 1_800_000_000;
        let days = |count: i64| (now - count * 24 * 60 * 60).to_string();
        assert!(!past_keeping(&days(29), now));
        assert!(!past_keeping(&days(30), now));
        assert!(past_keeping(&days(31), now));
        assert!(!past_keeping("notes", now), "nothing the server did not make is touched");
    }

    #[test]
    fn a_file_is_renamed_by_the_tags_it_is_given_and_keeps_its_extension() {
        let tags = EditedTags {
            title: Some("Quiet Harbour".to_string()),
            artists: vec!["Amber Field".to_string()],
            track: Some(1),
            ..EditedTags::default()
        };
        assert_eq!(
            renamed(
                "{track} - {artist} - {title}",
                &tags,
                Path::new("A/old name.flac")
            )
            .expect("named"),
            "01 - Amber Field - Quiet Harbour.flac"
        );
        assert!(renamed("{mood}", &tags, Path::new("A/x.flac")).is_err());
    }
}
