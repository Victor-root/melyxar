//! Taking up what an Emby or Jellyfin server kept of each account: the reading
//! is the crate `melyxar-import`'s, how its items are found here is
//! `melyxar_import::matching`'s, and the writing is the database's.
//!
//! First a preview, which writes nothing: the accounts of the other server
//! and the one each would become here, how much of the other server was
//! found here, what would be taken up and what was found nowhere. Then the
//! administrator's choices: which account here each account there becomes,
//! one that exists or one made with a password they give, and which
//! collections are kept. Both read the backup again, so what is taken up is
//! what the preview showed.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use melyxar_core::id::{LibraryId, UserId, WorkId};
use melyxar_core::time::Millis;
use melyxar_core::user::{Permissions, User};
use melyxar_core::work::WorkKind;
use melyxar_database::import::{TakenListen, TakenProgress, WorkToMatch};
use melyxar_import::matching::{self, Matches, Ours};
use melyxar_import::{Backup, Kind, Mark, Refusal, Server};
use serde::{Deserialize, Serialize};

use crate::accounts;
use crate::{AppError, AppState};

/// The most items found nowhere a preview lists by name; the rest are counted.
const MOST_LISTED_AS_LOST: usize = 300;

/// Why nothing could be taken up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The path given is not an absolute one.
    PathNeeded,
    NotFound,
    NotAServer,
    /// A server older than what is read, with the version to upgrade to.
    TooOld(Server),
    Unreadable(String),
    /// An account that was to be made could not be.
    Account { name: String, why: accounts::Refused },
}

#[derive(Debug, thiserror::Error)]
pub enum Trouble {
    #[error("refused: {0:?}")]
    Refused(Refused),
    #[error(transparent)]
    Failed(#[from] AppError),
}

impl From<melyxar_database::DatabaseError> for Trouble {
    fn from(error: melyxar_database::DatabaseError) -> Self {
        Self::Failed(error.into())
    }
}

impl From<Refusal> for Trouble {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(match refusal {
            Refusal::NotFound(_) => Refused::NotFound,
            Refusal::NotAServer => Refused::NotAServer,
            Refusal::TooOld { server } => Refused::TooOld(server),
            Refusal::Unreadable(why) => Refused::Unreadable(why),
        })
    }
}

/// An account of the other server, and the one here it would become.
#[derive(Debug, Clone, Serialize)]
pub struct AccountSeen {
    pub key: String,
    pub name: String,
    pub administrator: bool,
    pub disabled: bool,
    /// The account here that goes by the same name, if one does.
    pub here: Option<UserSeen>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserSeen {
    pub id: UserId,
    pub name: String,
}

/// How many items of a kind the other server knew, and how many were found here.
#[derive(Debug, Clone, Serialize)]
pub struct Found {
    pub kind: Kind,
    pub found: usize,
    pub total: usize,
}

/// What would be taken up, all accounts together, of what was found here.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Taken {
    pub watched: usize,
    pub left_partway: usize,
    pub favourites: usize,
    pub listens: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListSeen {
    pub name: String,
    /// The name of the account it belongs to over there.
    pub owner: Option<String>,
    pub items: usize,
    pub found: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CollectionSeen {
    pub name: String,
    pub items: usize,
    pub found: usize,
    pub automatic: Option<bool>,
}

/// Something an account did something with that was found nowhere here.
#[derive(Debug, Clone, Serialize)]
pub struct Lost {
    pub kind: Kind,
    pub name: String,
    pub year: Option<i32>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Preview {
    pub server: Server,
    pub oldest_read: &'static str,
    pub accounts: Vec<AccountSeen>,
    pub found: Vec<Found>,
    pub taken: Taken,
    pub playlists: Vec<ListSeen>,
    pub collections: Vec<CollectionSeen>,
    pub lost: Vec<Lost>,
    pub lost_count: usize,
}

/// What becomes of one account of the other server.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "becomes", rename_all = "snake_case")]
pub enum AccountChoice {
    /// What it did goes to an account that already exists here.
    Existing { key: String, user: UserId },
    /// An account is made for it, under this name and password.
    New { key: String, name: String, password: String },
    /// Nothing of it is taken up.
    Nothing { key: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choices {
    pub accounts: Vec<AccountChoice>,
    /// The collections taken up, by their name over there.
    pub collections: Vec<String>,
}

/// What was taken up.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub accounts_made: usize,
    pub accounts_joined: usize,
    pub taken: Taken,
    pub playlists: usize,
    pub collections: usize,
    /// Playlists and collections left as they were because one of the same
    /// name was already here.
    pub left_alone: Vec<String>,
}

/// A backup read and its items looked for here.
struct Read {
    backup: Backup,
    matches: Matches,
    libraries: HashMap<String, String>,
}

impl Read {
    fn work(&self, item: &str) -> Option<WorkId> {
        self.matches.get(item).and_then(|(work, _)| work.parse().ok())
    }
}

/// Reads the data folder of an Emby or Jellyfin server, or a copy of it, and
/// says what would be taken up from it.
pub async fn preview(state: &AppState, from: &str) -> Result<Preview, Trouble> {
    let read = read(state, from).await?;
    preview_of(state, &read).await
}

async fn preview_of(state: &AppState, read: &Read) -> Result<Preview, Trouble> {
    let here: HashMap<String, User> = state
        .database()
        .list_users()
        .await?
        .into_iter()
        .map(|user| (user.name.to_lowercase(), user))
        .collect();
    let backup = &read.backup;
    let name_of: HashMap<&str, &str> =
        backup.accounts.iter().map(|account| (account.key.as_str(), account.name.as_str())).collect();
    let accounts = backup
        .accounts
        .iter()
        .map(|account| AccountSeen {
            key: account.key.clone(),
            name: account.name.clone(),
            administrator: account.administrator,
            disabled: account.disabled,
            here: here
                .get(&account.name.to_lowercase())
                .map(|user| UserSeen { id: user.id, name: user.name.clone() }),
        })
        .collect();

    let mut found: Vec<Found> = Vec::new();
    for kind in [Kind::Movie, Kind::Series, Kind::Episode, Kind::Video, Kind::Song, Kind::Album, Kind::Artist] {
        let of_kind: Vec<_> = backup.items.iter().filter(|item| item.kind == kind).collect();
        if !of_kind.is_empty() {
            found.push(Found {
                kind,
                found: of_kind.iter().filter(|item| read.matches.contains_key(&item.key)).count(),
                total: of_kind.len(),
            });
        }
    }

    let mut taken = Taken::default();
    let mut lost_keys: Vec<&str> = Vec::new();
    let mut seen_lost: HashSet<&str> = HashSet::new();
    let kind_of: HashMap<&str, Kind> = backup.items.iter().map(|item| (item.key.as_str(), item.kind)).collect();
    for mark in &backup.marks {
        if !read.matches.contains_key(&mark.item) {
            if seen_lost.insert(mark.item.as_str()) {
                lost_keys.push(mark.item.as_str());
            }
            continue;
        }
        match kind_of.get(mark.item.as_str()) {
            Some(Kind::Song) => taken.listens += usize::from(listens_of(mark) > 0),
            Some(Kind::Movie | Kind::Episode | Kind::Video) => {
                taken.watched += usize::from(mark.played);
                taken.left_partway += usize::from(!mark.played && mark.position_ms > 0);
            }
            _ => {}
        }
        taken.favourites += usize::from(mark.favorite);
    }
    let items: HashMap<&str, &melyxar_import::Item> =
        backup.items.iter().map(|item| (item.key.as_str(), item)).collect();
    let lost: Vec<Lost> = lost_keys
        .iter()
        .filter_map(|key| items.get(key))
        .take(MOST_LISTED_AS_LOST)
        .map(|item| Lost { kind: item.kind, name: item.name.clone(), year: item.year, path: item.path.clone() })
        .collect();

    let found_of = |items: &[String]| items.iter().filter(|item| read.matches.contains_key(*item)).count();
    Ok(Preview {
        server: backup.server,
        oldest_read: backup.server.oldest_read(),
        accounts,
        found,
        taken,
        playlists: backup
            .playlists
            .iter()
            .map(|playlist| ListSeen {
                name: playlist.name.clone(),
                owner: playlist.owner.as_deref().and_then(|owner| name_of.get(owner)).map(|name| name.to_string()),
                items: playlist.items.len(),
                found: found_of(&playlist.items),
            })
            .collect(),
        collections: backup
            .collections
            .iter()
            .map(|collection| CollectionSeen {
                name: collection.name.clone(),
                items: collection.items.len(),
                found: found_of(&collection.items),
                automatic: collection.automatic,
            })
            .collect(),
        lost_count: lost_keys.len(),
        lost,
    })
}

/// Takes up what the administrator chose, `who` being the one taking it up.
pub async fn take_up(state: &AppState, who: &User, from: &str, choices: &Choices) -> Result<Report, Trouble> {
    let read = read(state, from).await?;
    taken_up(state, who, &read, choices).await
}

async fn taken_up(state: &AppState, who: &User, read: &Read, choices: &Choices) -> Result<Report, Trouble> {
    let backup = &read.backup;
    let database = state.database();
    let mut report = Report::default();

    let mut becomes: HashMap<String, UserId> = HashMap::new();
    for choice in &choices.accounts {
        match choice {
            AccountChoice::Existing { key, user } => {
                if database.user(*user).await?.is_some() {
                    becomes.insert(key.clone(), *user);
                    report.accounts_joined += 1;
                }
            }
            AccountChoice::New { key, name, password } => {
                let Some(account) = backup.accounts.iter().find(|account| &account.key == key) else {
                    continue;
                };
                let made = accounts::create_account(state, name, password, &rights_of(account, read))
                    .await
                    .map_err(|trouble| match trouble {
                        accounts::Trouble::Refused(why) => {
                            Trouble::Refused(Refused::Account { name: name.clone(), why })
                        }
                        accounts::Trouble::Failed(error) => Trouble::Failed(error),
                    })?;
                if account.audio_language.is_some() || account.subtitle_language.is_some() {
                    let mut preferences = crate::preferences::of(state, made.id).await?;
                    preferences.preferred_audio_language = account.audio_language.clone();
                    preferences.preferred_subtitle_language = account.subtitle_language.clone();
                    crate::preferences::save(state, made.id, preferences).await?;
                }
                becomes.insert(key.clone(), made.id);
                report.accounts_made += 1;
            }
            AccountChoice::Nothing { .. } => {}
        }
    }

    let kind_of: HashMap<&str, Kind> = backup.items.iter().map(|item| (item.key.as_str(), item.kind)).collect();
    for (key, user) in &becomes {
        let marks: Vec<&Mark> = backup.marks.iter().filter(|mark| &mark.account == key).collect();
        let (progress, listens, favourites) = what_to_take_up(read, &kind_of, &marks);
        report.taken.watched += progress.iter().filter(|one| one.watched).count();
        report.taken.left_partway += progress.iter().filter(|one| !one.watched && one.position.get() > 0).count();
        report.taken.listens += listens.len();
        report.taken.favourites += favourites.len();
        database.take_up_progress(*user, &progress).await?;
        database.take_up_listens(*user, &listens).await?;
        database.take_up_favourites(*user, &favourites).await?;
    }

    for playlist in &backup.playlists {
        // A playlist shared with everybody goes to the one taking it up.
        let owner = match &playlist.owner {
            Some(owner) => match becomes.get(owner) {
                Some(user) => *user,
                None => continue,
            },
            None => who.id,
        };
        let works: Vec<(WorkId, Kind)> = playlist
            .items
            .iter()
            .filter_map(|item| Some((read.work(item)?, *kind_of.get(item.as_str())?)))
            .collect();
        let songs: Vec<WorkId> = works.iter().filter(|(_, kind)| *kind == Kind::Song).map(|(work, _)| *work).collect();
        let videos: Vec<WorkId> = works.iter().filter(|(_, kind)| *kind != Kind::Song).map(|(work, _)| *work).collect();
        if !songs.is_empty() {
            if database.music_playlists(owner, None).await?.iter().any(|kept| kept.name == playlist.name) {
                report.left_alone.push(playlist.name.clone());
            } else {
                let id = database.create_music_playlist(owner, &playlist.name).await?;
                database.add_to_music_playlist(owner, id, &songs).await?;
                report.playlists += 1;
            }
        }
        if !videos.is_empty() {
            if database.playlists(owner, None).await?.iter().any(|kept| kept.name == playlist.name) {
                report.left_alone.push(playlist.name.clone());
            } else {
                let id = database.create_playlist(owner, &playlist.name).await?;
                database.add_to_playlist(owner, id, &videos).await?;
                report.playlists += 1;
            }
        }
    }

    let chosen: HashSet<&str> = choices.collections.iter().map(String::as_str).collect();
    let kept: HashSet<String> = database
        .collections(who.id, None, true)
        .await?
        .into_iter()
        .map(|collection| collection.name.to_lowercase())
        .collect();
    for collection in backup.collections.iter().filter(|collection| chosen.contains(collection.name.as_str())) {
        let works: Vec<WorkId> = collection.items.iter().filter_map(|item| read.work(item)).collect();
        if works.is_empty() {
            continue;
        }
        if kept.contains(&collection.name.to_lowercase()) {
            report.left_alone.push(collection.name.clone());
            continue;
        }
        let id = database
            .create_collection(&collection.name, &melyxar_library::naming::sort_title(&collection.name))
            .await?;
        database.add_to_collection(id, &works).await?;
        report.collections += 1;
    }

    tracing::info!(
        from = ?backup.server,
        accounts_made = report.accounts_made,
        accounts_joined = report.accounts_joined,
        watched = report.taken.watched,
        left_partway = report.taken.left_partway,
        favourites = report.taken.favourites,
        listens = report.taken.listens,
        playlists = report.playlists,
        collections = report.collections,
        "took up what another server kept"
    );
    Ok(report)
}

/// How often a mark says a song was listened to: once at least when it
/// says it was played.
fn listens_of(mark: &Mark) -> u32 {
    mark.play_count.max(u32::from(mark.played))
}

/// What one account did, as it is written here: one entry a work, the copies
/// of a work over there merged, the latest and the most kept.
fn what_to_take_up(
    read: &Read,
    kind_of: &HashMap<&str, Kind>,
    marks: &[&Mark],
) -> (Vec<TakenProgress>, Vec<TakenListen>, Vec<WorkId>) {
    let mut progress: HashMap<WorkId, TakenProgress> = HashMap::new();
    let mut listens: HashMap<WorkId, TakenListen> = HashMap::new();
    let mut favourites: Vec<WorkId> = Vec::new();
    for mark in marks {
        let Some(work) = read.work(&mark.item) else {
            continue;
        };
        if mark.favorite && !favourites.contains(&work) {
            favourites.push(work);
        }
        match kind_of.get(mark.item.as_str()) {
            Some(Kind::Movie | Kind::Episode | Kind::Video) => {
                if !mark.played && mark.position_ms == 0 && mark.play_count == 0 {
                    continue;
                }
                let taken = TakenProgress {
                    work,
                    watched: mark.played,
                    play_count: mark.play_count,
                    position: Millis::new(mark.position_ms),
                    last_played: mark.last_played,
                };
                progress
                    .entry(work)
                    .and_modify(|kept| {
                        let count = kept.play_count.max(taken.play_count);
                        if taken.last_played > kept.last_played {
                            *kept = taken.clone();
                        }
                        kept.play_count = count;
                    })
                    .or_insert(taken);
            }
            Some(Kind::Song) => {
                let count = listens_of(mark);
                if count == 0 {
                    continue;
                }
                listens
                    .entry(work)
                    .and_modify(|kept| {
                        kept.listens = kept.listens.max(count);
                        kept.last_listened = kept.last_listened.max(mark.last_played);
                    })
                    .or_insert(TakenListen { song: work, listens: count, last_listened: mark.last_played });
            }
            _ => {}
        }
    }
    (progress.into_values().collect(), listens.into_values().collect(), favourites)
}

/// The rights an account made for an account over there is given: the same,
/// over the libraries here its libraries there turned out to be.
fn rights_of(account: &melyxar_import::Account, read: &Read) -> Permissions {
    let mut rights = if account.administrator { Permissions::administrator() } else { Permissions::viewer() };
    if !account.administrator {
        rights.may_delete = account.may_delete;
        rights.may_download = account.may_download;
        if let Some(libraries) = &account.libraries {
            rights.sees_every_library = false;
            rights.allowed_libraries = libraries
                .iter()
                .filter_map(|library| read.libraries.get(library))
                .filter_map(|library| library.parse::<LibraryId>().ok())
                .collect();
        }
    }
    rights
}

/// Reads the backup, from a copy made in the cache and let go afterwards, and
/// looks for its items here.
async fn read(state: &AppState, from: &str) -> Result<Read, Trouble> {
    let from = PathBuf::from(from.trim());
    if !from.is_absolute() {
        return Err(Trouble::Refused(Refused::PathNeeded));
    }
    let work = state.config().directories.cache.join("import").join(uuid::Uuid::now_v7().simple().to_string());
    let backup = melyxar_import::read(&from, &work).await;
    let _ = tokio::fs::remove_dir_all(&work).await;
    looked_for(state, backup?).await
}

/// Looks for the items of a backup here.
async fn looked_for(state: &AppState, backup: Backup) -> Result<Read, Trouble> {
    let works = state.database().works_to_match_against().await?;
    let looked_for = backup.clone();
    let ours: Vec<Ours> = works.iter().filter_map(ours_of).collect();
    let (matches, libraries) = tokio::task::spawn_blocking(move || {
        let matches = matching::matched(&looked_for.items, &ours);
        let libraries = matching::libraries(&looked_for.libraries, &looked_for.items, &ours, &matches);
        (matches, libraries)
    })
    .await
    .map_err(|error| AppError::Directory(std::io::Error::other(error)))?;
    Ok(Read { backup, matches, libraries })
}

fn ours_of(work: &WorkToMatch) -> Option<Ours> {
    let kind = match work.kind {
        WorkKind::Movie => Kind::Movie,
        WorkKind::Series => Kind::Series,
        WorkKind::Season => Kind::Season,
        WorkKind::Episode => Kind::Episode,
        WorkKind::Video => Kind::Video,
        WorkKind::Song => Kind::Song,
        WorkKind::Album => Kind::Album,
        WorkKind::Artist => Kind::Artist,
        WorkKind::Folder | WorkKind::Photo => return None,
    };
    Some(Ours {
        id: work.id.to_db_string(),
        kind,
        library: work.library.to_db_string(),
        title: work.title.clone(),
        year: work.year,
        paths: work.paths.clone(),
        ids: work.external_ids.clone(),
        number: work.number,
        parent: work.parent.map(|parent| parent.to_db_string()),
        artists: work.artists.clone(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use melyxar_core::time::now;
    use melyxar_core::work::PlaybackState;
    use melyxar_import::{Account, Collection, Item, Library, Playlist};
    use time::macros::datetime;

    use super::*;

    fn film(key: &str, name: &str, path: Option<&str>, ids: &[(&str, &str)]) -> Item {
        Item {
            key: key.into(),
            kind: Kind::Movie,
            name: name.into(),
            year: Some(2011),
            path: path.map(str::to_string),
            ids: ids.iter().map(|(provider, id)| (provider.to_string(), id.to_string())).collect(),
            season: None,
            episode: None,
            parent: None,
            artists: Vec::new(),
        }
    }

    fn mark(account: &str, item: &str) -> Mark {
        Mark {
            account: account.into(),
            item: item.into(),
            played: false,
            play_count: 0,
            position_ms: 0,
            last_played: None,
            favorite: false,
        }
    }

    /// Ada and Ben on an Emby whose films folder was elsewhere: a film found
    /// by its file, one by its identifier, one found nowhere.
    fn a_backup() -> Backup {
        let account = |key: &str, name: &str, administrator: bool, libraries: Option<Vec<String>>| Account {
            key: key.into(),
            name: name.into(),
            administrator,
            disabled: false,
            may_delete: false,
            may_download: true,
            libraries,
            audio_language: (name == "Ben").then(|| "fre".to_string()),
            subtitle_language: None,
        };
        let watched = Mark {
            played: true,
            play_count: 2,
            last_played: Some(datetime!(2024-06-01 10:00 UTC)),
            ..mark("1", "10")
        };
        let liked = Mark { favorite: true, ..mark("1", "11") };
        let left = Mark { position_ms: 600_000, last_played: Some(datetime!(2024-06-02 10:00 UTC)), ..mark("2", "11") };
        let lost = Mark { played: true, ..mark("1", "12") };
        Backup {
            server: Server::Emby,
            accounts: vec![account("1", "ada", true, None), account("2", "Ben", false, Some(vec!["L".into()]))],
            libraries: vec![Library { key: "L".into(), name: "Films".into(), folders: vec!["/old/Films".into()] }],
            items: vec![
                film("10", "Harbour Lights", Some("/old/Films/Harbour Lights.mkv"), &[]),
                film("11", "Northern Road", None, &[("tmdb", "2002")]),
                film("12", "Missing Piece", Some("/old/Films/Missing Piece.mkv"), &[]),
            ],
            marks: vec![watched, liked, left, lost],
            playlists: vec![Playlist { name: "Evening".into(), owner: Some("1".into()), items: vec!["10".into(), "11".into()] }],
            collections: vec![
                Collection { name: "Rainy days".into(), items: vec!["10".into()], automatic: Some(false) },
                Collection { name: "Saga".into(), items: vec!["11".into()], automatic: Some(true) },
            ],
        }
    }

    async fn a_server(directory: &Path) -> (AppState, User, WorkId, WorkId, LibraryId) {
        let (config, database, library) =
            crate::a_test_server(directory, "movies", vec![("disk", PathBuf::from("/mnt/films"))]).await;
        let harbour = database
            .create_work(library.id, WorkKind::Movie, "Harbour Lights", "harbour lights", Some(2011))
            .await
            .unwrap();
        database.insert_source(harbour.id, library.roots[0].id, Path::new("Harbour Lights.mkv"), 10, now()).await.unwrap();
        let road = database
            .create_work(library.id, WorkKind::Movie, "Northern Road", "northern road", Some(2004))
            .await
            .unwrap();
        database.set_work_external_id(road.id, "tmdb", "2002").await.unwrap();
        let ada = database.create_user("Ada", None, &Permissions::administrator()).await.unwrap();
        (AppState::new(config, database, None, None), ada, harbour.id, road.id, library.id)
    }

    #[tokio::test]
    async fn the_preview_says_who_becomes_whom_what_was_found_and_what_was_not() {
        let directory = tempfile::tempdir().unwrap();
        let (state, ada, _, _, _) = a_server(directory.path()).await;
        let read = looked_for(&state, a_backup()).await.unwrap();
        let preview = preview_of(&state, &read).await.unwrap();

        assert_eq!(preview.oldest_read, "4.8");
        assert_eq!(preview.accounts[0].here.as_ref().map(|user| user.id), Some(ada.id));
        assert!(preview.accounts[1].here.is_none());
        assert_eq!((preview.found[0].kind, preview.found[0].found, preview.found[0].total), (Kind::Movie, 2, 3));
        assert_eq!((preview.taken.watched, preview.taken.left_partway, preview.taken.favourites), (1, 1, 1));
        assert_eq!(preview.lost_count, 1);
        assert_eq!(preview.lost[0].name, "Missing Piece");
        assert_eq!((preview.playlists[0].owner.as_deref(), preview.playlists[0].found), (Some("ada"), 2));
        assert_eq!(preview.collections[1].automatic, Some(true));
    }

    #[tokio::test]
    async fn taking_up_writes_what_each_account_did_into_the_account_it_becomes() {
        let directory = tempfile::tempdir().unwrap();
        let (state, ada, harbour, road, films) = a_server(directory.path()).await;
        let read = looked_for(&state, a_backup()).await.unwrap();
        let choices = Choices {
            accounts: vec![
                AccountChoice::Existing { key: "1".into(), user: ada.id },
                AccountChoice::New { key: "2".into(), name: "Ben".into(), password: "quiet harbour lights".into() },
            ],
            collections: vec!["Rainy days".into()],
        };
        let report = taken_up(&state, &ada, &read, &choices).await.unwrap();
        assert_eq!((report.accounts_joined, report.accounts_made), (1, 1));
        assert_eq!((report.taken.watched, report.taken.left_partway, report.taken.favourites), (1, 1, 1));
        assert_eq!((report.playlists, report.collections), (1, 1));

        let database = state.database();
        let watched = database.playback_progress(ada.id, harbour).await.unwrap().unwrap();
        assert_eq!((watched.state, watched.play_count), (PlaybackState::Watched, 2));
        assert!(database.playback_progress(ada.id, road).await.unwrap().is_none());

        let (ben, _) = database.user_by_name("Ben").await.unwrap().unwrap();
        assert!(!ben.permissions.is_administrator && !ben.permissions.sees_every_library);
        assert_eq!(ben.permissions.allowed_libraries, vec![films]);
        assert_eq!(ben.preferences.preferred_audio_language.as_deref(), Some("fre"));
        let left = database.playback_progress(ben.id, road).await.unwrap().unwrap();
        assert_eq!((left.state, left.position), (PlaybackState::InProgress, Millis::new(600_000)));

        let playlists = database.playlists(ada.id, None).await.unwrap();
        assert_eq!((playlists[0].name.as_str(), playlists[0].held), ("Evening", 2));
        let collections = database.collections(ada.id, None, true).await.unwrap();
        assert_eq!(collections.iter().map(|collection| collection.name.as_str()).collect::<Vec<_>>(), vec!["Rainy days"]);

        // Taken up twice, nothing is made twice.
        let again = taken_up(
            &state,
            &ada,
            &read,
            &Choices {
                accounts: vec![AccountChoice::Existing { key: "1".into(), user: ada.id }],
                collections: vec!["Rainy days".into()],
            },
        )
        .await
        .unwrap();
        assert_eq!((again.playlists, again.collections), (0, 0));
        assert_eq!(again.left_alone, vec!["Evening", "Rainy days"]);
    }

    #[tokio::test]
    async fn a_relative_path_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let (state, _, _, _, _) = a_server(directory.path()).await;
        assert!(matches!(preview(&state, "backups/emby").await, Err(Trouble::Refused(Refused::PathNeeded))));
    }
}
