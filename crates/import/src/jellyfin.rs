//! Jellyfin, from 10.9.11 on.
//!
//! Its data folder holds `data/jellyfin.db`, the accounts with their rights
//! and preferences, and up to 10.10 `data/library.db`, every item and what
//! each account did with it. 10.11 moved the items into `jellyfin.db` and left
//! the old file beside it as `library.db.old`, never read here; 12 gave the
//! members of playlists and collections a table of their own, where they were
//! written inside the list until then. A library is a folder under
//! `root/default`, with one `.mblink` file naming each folder of media.
//!
//! From 10.11 Jellyfin also makes a backup of its own: a zip of the same
//! tables written out as JSON, one file a table, beside a copy of its `root`
//! folder. Unzipped, it is read to the same result as the database it was
//! made from, both being brought to the same rows first.
//!
//! An item's kind is the full name of its .NET type. What an account did with
//! an item is kept under keys the item works out from its identifiers, so two
//! copies of a film share it: up to 10.10 each item names the one key it
//! reads, and from 10.11 the rows are tied to the item, several of them when
//! its keys changed over time.

mod export;
mod sqlite;

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::guid;
use crate::{Account, Backup, Collection, Item, Kind, Library, Mark, Playlist, Refusal, Server, TICKS_A_MILLISECOND};

const MOVIE: &str = "MediaBrowser.Controller.Entities.Movies.Movie";
const SERIES: &str = "MediaBrowser.Controller.Entities.TV.Series";
const SEASON: &str = "MediaBrowser.Controller.Entities.TV.Season";
const EPISODE: &str = "MediaBrowser.Controller.Entities.TV.Episode";
const VIDEO: &str = "MediaBrowser.Controller.Entities.Video";
const SONG: &str = "MediaBrowser.Controller.Entities.Audio.Audio";
const ALBUM: &str = "MediaBrowser.Controller.Entities.Audio.MusicAlbum";
const ARTIST: &str = "MediaBrowser.Controller.Entities.Audio.MusicArtist";
const LIBRARY: &str = "MediaBrowser.Controller.Entities.CollectionFolder";
const PLAYLIST: &str = "MediaBrowser.Controller.Playlists.Playlist";
const COLLECTION: &str = "MediaBrowser.Controller.Entities.Movies.BoxSet";

/// The kinds taken up as items.
const KINDS: [&str; 8] = [MOVIE, SERIES, SEASON, EPISODE, VIDEO, SONG, ALBUM, ARTIST];
/// The other kinds read: the libraries, and the lists whose JSON alone holds
/// their owner, and their members until 12.
const OTHERS: [&str; 3] = [LIBRARY, PLAYLIST, COLLECTION];
const LISTS: [&str; 2] = [PLAYLIST, COLLECTION];

/// The rights read, among the `Kind` numbers of `Permissions`.
const IS_ADMINISTRATOR: i64 = 0;
const IS_DISABLED: i64 = 2;
const ENABLE_CONTENT_DELETION: i64 = 10;
const ENABLE_CONTENT_DOWNLOADING: i64 = 11;
const ENABLE_ALL_FOLDERS: i64 = 16;
/// The libraries an account may see when not all of them, in `Preferences`.
const ENABLED_FOLDERS: i64 = 5;

/// Where the files of a Jellyfin server are.
pub(crate) enum Found {
    /// Its data folder, or a copy: the `data` folder and the libraries.
    Folder { data: PathBuf, root: PathBuf },
    /// The backup Jellyfin made of itself, unzipped: its tables and the
    /// libraries.
    Export { database: PathBuf, root: PathBuf },
}

impl Found {
    /// The data folder of a Jellyfin server given it or its `data` folder, or
    /// the folder its own backup was unzipped into.
    pub(crate) fn at(from: &Path) -> Option<Self> {
        if from.join("manifest.json").is_file() && from.join("Database").is_dir() {
            return Some(Self::Export { database: from.join("Database"), root: from.join("Root") });
        }
        let folder = if from.join("data/jellyfin.db").is_file() {
            from.to_path_buf()
        } else if from.join("jellyfin.db").is_file() {
            from.parent()?.to_path_buf()
        } else {
            return None;
        };
        Some(Self::Folder { data: folder.join("data"), root: folder.join("root") })
    }
}

pub(crate) async fn read(found: &Found, work: &Path) -> Result<Backup, Refusal> {
    let (tables, root) = match found {
        Found::Folder { data, root } => (sqlite::tables(data, work).await?, root),
        Found::Export { database, root } => (export::tables(database.clone()).await?, root),
    };
    Ok(backup(tables, root).await)
}

/// The rows read, the same whether they come from Jellyfin's databases or
/// from its own backup, every identifier already in one form.
struct Tables {
    users: Vec<User>,
    permissions: Vec<Setting<bool>>,
    preferences: Vec<Setting<String>>,
    /// The items of the kinds read, in no order.
    items: Vec<Stored>,
    /// The identifiers at providers of each item.
    ids: HashMap<String, Vec<(String, String)>>,
    plays: Vec<Played>,
    /// The members of every list, from 12; until then they are in the list's
    /// own JSON.
    members: Option<Vec<Member>>,
}

/// A row of `Users`. Its password is never read.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct User {
    #[serde(deserialize_with = "key")]
    id: String,
    username: String,
    /// The number the accounts are named by in `library.db`, up to 10.10.
    internal_id: i64,
    audio_language_preference: Option<String>,
    subtitle_language_preference: Option<String>,
}

/// A row of `Permissions` or `Preferences`, one right or one preference of
/// one account.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Setting<T> {
    #[serde(default, deserialize_with = "maybe_key")]
    user_id: Option<String>,
    kind: i64,
    value: T,
}

/// An item as kept, in `BaseItems` or up to 10.10 `TypedBaseItems`.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Stored {
    #[serde(deserialize_with = "key")]
    id: String,
    #[serde(rename = "Type")]
    kind: String,
    name: Option<String>,
    path: Option<String>,
    production_year: Option<i32>,
    index_number: Option<i32>,
    parent_index_number: Option<i32>,
    #[serde(default, deserialize_with = "maybe_key")]
    series_id: Option<String>,
    #[serde(default, deserialize_with = "maybe_key")]
    parent_id: Option<String>,
    artists: Option<String>,
    album_artists: Option<String>,
    /// The item written out as JSON, kept for the lists alone.
    data: Option<String>,
}

/// A row of `UserData`: what an account did with an item.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Played {
    #[serde(deserialize_with = "key")]
    item_id: String,
    #[serde(deserialize_with = "key")]
    user_id: String,
    played: bool,
    play_count: i64,
    is_favorite: bool,
    playback_position_ticks: i64,
    #[serde(default, deserialize_with = "date")]
    last_played_date: Option<OffsetDateTime>,
}

/// A row of `LinkedChildren`, from 12: one member of a list.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Member {
    #[serde(deserialize_with = "key")]
    parent_id: String,
    #[serde(deserialize_with = "key")]
    child_id: String,
    sort_order: i64,
}

/// What a list's JSON holds that is read.
#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct List {
    owner_user_id: Option<String>,
    linked_children: Option<Vec<Linked>>,
}

/// A member of a list as its JSON names it until 12: by the item it was
/// last found to be, its file, or for an item without a file its
/// identifier.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Linked {
    item_id: Option<String>,
    path: Option<String>,
    library_item_id: Option<String>,
}

/// A GUID, written in any of the ways Jellyfin writes them.
fn key<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let text = String::deserialize(deserializer)?;
    guid::from_text(&text).ok_or_else(|| D::Error::custom(format!("not a GUID: {text}")))
}

fn maybe_key<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.as_deref().and_then(guid::from_text))
}

fn date<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<OffsetDateTime>, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.as_deref().and_then(date_of))
}

/// A date as .NET writes it, `2023-11-14 22:13:20.1234567`, with a `T` in
/// the middle in a backup, with or without a fraction of a second, and with
/// or without an offset. One without is in UTC, as Jellyfin keeps them all.
fn date_of(text: &str) -> Option<OffsetDateTime> {
    let text = text.trim();
    if !matches!(text.as_bytes().get(10), Some(b' ' | b'T')) {
        return None;
    }
    let (day, time) = (text.get(..10)?, text.get(11..)?);
    let offset = if time.ends_with(['Z', 'z']) || time.contains(['+', '-']) { "" } else { "Z" };
    OffsetDateTime::parse(&format!("{day}T{time}{offset}"), &Rfc3339).ok()
}

impl<T> Setting<T> {
    fn is(&self, user: &User, kind: i64) -> bool {
        self.kind == kind && self.user_id.as_deref() == Some(user.id.as_str())
    }
}

impl Stored {
    /// The item if it is of a kind read, its JSON let go unless it is a list.
    fn kept(mut self) -> Option<Self> {
        if !KINDS.contains(&self.kind.as_str()) && !OTHERS.contains(&self.kind.as_str()) {
            return None;
        }
        if !LISTS.contains(&self.kind.as_str()) {
            self.data = None;
        }
        Some(self)
    }
}

async fn backup(mut tables: Tables, root: &Path) -> Backup {
    tables.items.sort_by(|one, other| one.id.cmp(&other.id));
    let accounts = accounts(&tables);
    let libraries = libraries(&tables.items, root).await;
    let items = items(&tables);
    let marks = marks(tables.plays, &accounts, &items);
    let (playlists, collections) = lists(&tables.items, tables.members.as_deref(), &accounts, &items);
    Backup { server: Server::Jellyfin, accounts, libraries, items, marks, playlists, collections }
}

fn accounts(tables: &Tables) -> Vec<Account> {
    let mut users: Vec<&User> = tables.users.iter().collect();
    users.sort_by_key(|user| user.internal_id);
    users
        .into_iter()
        .map(|user| {
            let may = |kind: i64| tables.permissions.iter().any(|right| right.is(user, kind) && right.value);
            let libraries = (!may(ENABLE_ALL_FOLDERS)).then(|| {
                tables
                    .preferences
                    .iter()
                    .find(|preference| preference.is(user, ENABLED_FOLDERS))
                    .map(|preference| preference.value.split(',').filter_map(guid::from_text).collect())
                    .unwrap_or_default()
            });
            let language = |written: &Option<String>| written.clone().filter(|value| !value.is_empty());
            Account {
                key: user.id.clone(),
                name: user.username.clone(),
                administrator: may(IS_ADMINISTRATOR),
                disabled: may(IS_DISABLED),
                may_delete: may(ENABLE_CONTENT_DELETION),
                may_download: may(ENABLE_CONTENT_DOWNLOADING),
                libraries,
                audio_language: language(&user.audio_language_preference),
                subtitle_language: language(&user.subtitle_language_preference),
            }
        })
        .collect()
}

async fn libraries(items: &[Stored], root: &Path) -> Vec<Library> {
    let mut libraries = Vec::new();
    for library in items.iter().filter(|item| item.kind == LIBRARY) {
        libraries.push(Library {
            key: library.id.clone(),
            name: library.name.clone().unwrap_or_default(),
            folders: folders(root, library.path.as_deref()).await,
        });
    }
    libraries.sort_by(|one, other| one.name.cmp(&other.name));
    libraries
}

/// The folders a library is made of: the item of a library is the folder of
/// that name under `root/default`, holding one `.mblink` file for each
/// folder of media, whose text is the folder.
async fn folders(root: &Path, library: Option<&str>) -> Vec<String> {
    let Some(name) = library
        .and_then(|path| path.rsplit(['/', '\\']).next())
        .filter(|name| !matches!(*name, "" | "." | ".."))
    else {
        return Vec::new();
    };
    let Ok(mut entries) = tokio::fs::read_dir(root.join("default").join(name)).await else {
        return Vec::new();
    };
    let mut folders = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("mblink")) {
            continue;
        }
        if let Ok(text) = tokio::fs::read_to_string(&path).await
            && !text.trim().is_empty()
        {
            folders.push(text.trim().to_string());
        }
    }
    folders.sort();
    folders
}

fn kind_of(name: &str) -> Option<Kind> {
    Some(match name {
        MOVIE => Kind::Movie,
        SERIES => Kind::Series,
        SEASON => Kind::Season,
        EPISODE => Kind::Episode,
        VIDEO => Kind::Video,
        SONG => Kind::Song,
        ALBUM => Kind::Album,
        ARTIST => Kind::Artist,
        _ => return None,
    })
}

fn items(tables: &Tables) -> Vec<Item> {
    let albums: HashSet<&str> =
        tables.items.iter().filter(|item| item.kind == ALBUM).map(|item| item.id.as_str()).collect();
    tables
        .items
        .iter()
        .filter_map(|stored| {
            let kind = kind_of(&stored.kind)?;
            // A song's album is the folder it sits in, when that is an album.
            let parent = match kind {
                Kind::Episode | Kind::Season => stored.series_id.clone(),
                Kind::Song => stored.parent_id.clone().filter(|parent| albums.contains(parent.as_str())),
                _ => None,
            };
            let (season, episode) = match kind {
                Kind::Episode => (stored.parent_index_number, stored.index_number),
                Kind::Season => (stored.index_number, None),
                _ => (None, None),
            };
            let mut ids = tables.ids.get(&stored.id).cloned().unwrap_or_default();
            ids.sort();
            Some(Item {
                key: stored.id.clone(),
                kind,
                name: stored.name.clone().unwrap_or_default(),
                year: stored.production_year,
                path: stored.path.clone(),
                ids,
                season,
                episode,
                parent,
                artists: if matches!(kind, Kind::Song | Kind::Album) { artists(stored) } else { Vec::new() },
            })
        })
        .collect()
}

/// The artists the album is credited to, or failing those every one that
/// plays on it, as Jellyfin writes them: names joined by `|`.
fn artists(stored: &Stored) -> Vec<String> {
    let names = |written: &Option<String>| -> Vec<String> {
        written
            .as_deref()
            .unwrap_or_default()
            .split('|')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect()
    };
    let credited = names(&stored.album_artists);
    if credited.is_empty() { names(&stored.artists) } else { credited }
}

/// What each account did with each item, once. When several rows of one item
/// disagree, Jellyfin 12 settles on the one played last, then the one played
/// most, then the one left furthest, and so does this.
fn marks(plays: Vec<Played>, accounts: &[Account], items: &[Item]) -> Vec<Mark> {
    let accounts: HashSet<&str> = accounts.iter().map(|account| account.key.as_str()).collect();
    let items: HashSet<&str> = items.iter().map(|item| item.key.as_str()).collect();
    let rank = |played: &Played| (played.last_played_date, played.play_count, played.playback_position_ticks);
    let mut kept: BTreeMap<(String, String), Played> = BTreeMap::new();
    for played in plays {
        if !accounts.contains(played.user_id.as_str()) || !items.contains(played.item_id.as_str()) {
            continue;
        }
        match kept.entry((played.user_id.clone(), played.item_id.clone())) {
            Entry::Vacant(entry) => {
                entry.insert(played);
            }
            Entry::Occupied(mut entry) => {
                if rank(&played) > rank(entry.get()) {
                    entry.insert(played);
                }
            }
        }
    }
    kept.into_values()
        .filter(|played| {
            played.played || played.play_count > 0 || played.is_favorite || played.playback_position_ticks > 0
        })
        .map(|played| Mark {
            account: played.user_id,
            item: played.item_id,
            played: played.played,
            play_count: u32::try_from(played.play_count).unwrap_or(0),
            position_ms: played.playback_position_ticks.max(0) / TICKS_A_MILLISECOND,
            last_played: played.last_played_date,
            favorite: played.is_favorite,
        })
        .collect()
}

/// The playlists and the collections, each with the items it holds in its
/// order. Jellyfin keeps no way to tell a collection it made itself from one
/// made by hand.
fn lists(
    stored: &[Stored],
    members: Option<&[Member]>,
    accounts: &[Account],
    items: &[Item],
) -> (Vec<Playlist>, Vec<Collection>) {
    let accounts: HashSet<&str> = accounts.iter().map(|account| account.key.as_str()).collect();
    let kept: HashSet<&str> = items.iter().map(|item| item.key.as_str()).collect();
    let by_path: HashMap<&str, &str> =
        items.iter().filter_map(|item| Some((item.path.as_deref()?, item.key.as_str()))).collect();
    let mut tabled = members.map(|members| {
        let mut sorted: Vec<&Member> =
            members.iter().filter(|member| kept.contains(member.child_id.as_str())).collect();
        sorted.sort_by_key(|member| member.sort_order);
        let mut tabled: HashMap<&str, Vec<String>> = HashMap::new();
        for member in sorted {
            tabled.entry(member.parent_id.as_str()).or_default().push(member.child_id.clone());
        }
        tabled
    });
    let mut playlists = Vec::new();
    let mut collections = Vec::new();
    for list in stored.iter().filter(|item| LISTS.contains(&item.kind.as_str())) {
        let written: List = list.data.as_deref().and_then(|data| serde_json::from_str(data).ok()).unwrap_or_default();
        let listed = match &mut tabled {
            Some(tabled) => tabled.remove(list.id.as_str()).unwrap_or_default(),
            None => written
                .linked_children
                .iter()
                .flatten()
                .filter_map(|linked| resolved(linked, &kept, &by_path))
                .collect(),
        };
        let name = list.name.clone().unwrap_or_default();
        if list.kind == PLAYLIST {
            playlists.push(Playlist {
                name,
                owner: written
                    .owner_user_id
                    .as_deref()
                    .and_then(guid::from_text)
                    .filter(|owner| accounts.contains(owner.as_str())),
                items: listed,
            });
        } else {
            collections.push(Collection { name, items: listed, automatic: None });
        }
    }
    playlists.sort_by(|one, other| one.name.cmp(&other.name));
    collections.sort_by(|one, other| one.name.cmp(&other.name));
    (playlists, collections)
}

/// The item a member written in a list's JSON is, found the way Jellyfin 12
/// found them when it moved them to their table: by the item it was last
/// found to be, then by its file, then by the identifier of an item without
/// a file.
fn resolved(linked: &Linked, kept: &HashSet<&str>, by_path: &HashMap<&str, &str>) -> Option<String> {
    let known = |written: &Option<String>| {
        written.as_deref().and_then(guid::from_text).filter(|key| kept.contains(key.as_str()))
    };
    known(&linked.item_id)
        .or_else(|| linked.path.as_deref().and_then(|path| by_path.get(path)).map(|key| key.to_string()))
        .or_else(|| known(&linked.library_item_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_read_as_dotnet_writes_them_and_in_utc_without_an_offset() {
        let at = |text: &str| date_of(text).map(|date| (date.unix_timestamp(), date.nanosecond()));
        assert_eq!(at("2023-11-14 22:13:20Z"), Some((1_700_000_000, 0)));
        assert_eq!(at("2023-11-14 22:13:20.1234567Z"), Some((1_700_000_000, 123_456_700)));
        assert_eq!(at("2023-11-14 22:13:20.5"), Some((1_700_000_000, 500_000_000)));
        assert_eq!(at("2023-11-14T22:13:20.5Z"), Some((1_700_000_000, 500_000_000)));
        assert_eq!(at("2023-11-14T23:13:20+01:00"), Some((1_700_000_000, 0)));
        assert_eq!(at("14/11/2023"), None);
        assert_eq!(at(""), None);
    }
}
