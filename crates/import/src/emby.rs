//! Emby, from 4.8 on.
//!
//! Its data folder holds `data/library.db`, every item and what each account
//! did with it, `data/users.db`, the accounts, and `config/users/<account>/`,
//! two small files of each account's rights (`policy.xml`) and preferences
//! (`config.xml`).
//!
//! An item's kind is a number. Emby publishes none of them: the ones below
//! were read on a real 4.9 database and match what other tools reading 4.8
//! found. What an account did with an item is kept once for every copy of
//! it, under a key the items share, so a film in two qualities is watched
//! once.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sqlx::Row;
use sqlx::sqlite::SqliteConnection;
use time::OffsetDateTime;

use crate::copy::{has_table, opened};
use crate::{Account, Backup, Collection, Item, Kind, Library, Mark, Playlist, Refusal, Server, TICKS_A_MILLISECOND};
use crate::{guid, provider};

const LIBRARY: i64 = 4;
const MOVIE: i64 = 5;
const SERIES: i64 = 6;
const SEASON: i64 = 7;
const EPISODE: i64 = 8;
const SONG: i64 = 11;
const ALBUM: i64 = 12;
const ARTIST: i64 = 13;
const PLAYLIST: i64 = 16;
const VIDEO: i64 = 18;

/// How an item is linked to the artists it is filed under, in `ItemLinks2`:
/// every artist that plays on it, and the ones the album is credited to.
const ARTISTS_LINK: i64 = 0;
const ALBUM_ARTISTS_LINK: i64 = 1;
/// How a library is linked to the folders it is made of.
const FOLDERS_LINK: i64 = 8;

/// Where the files of an Emby server are.
pub(crate) struct Found {
    library: PathBuf,
    users: PathBuf,
    config: PathBuf,
}

impl Found {
    /// The data folder of an Emby server given it, or its `data` folder.
    pub(crate) fn at(from: &Path) -> Option<Self> {
        let root = if from.join("data/users.db").is_file() {
            from.to_path_buf()
        } else if from.join("users.db").is_file() {
            from.parent()?.to_path_buf()
        } else {
            return None;
        };
        let data = root.join("data");
        data.join("library.db").is_file().then(|| Self {
            library: data.join("library.db"),
            users: data.join("users.db"),
            config: root.join("config/users"),
        })
    }
}

pub(crate) async fn read(found: &Found, work: &Path) -> Result<Backup, Refusal> {
    let mut library = opened(&found.library, work).await?;
    let mut users = opened(&found.users, work).await?;
    if has_table(&mut library, "TypedBaseItems").await?
        || !has_table(&mut library, "ItemExtradata").await?
        || !has_table(&mut library, "UserDataKeys2").await?
    {
        return Err(Refusal::TooOld { server: Server::Emby });
    }
    if !has_table(&mut library, "MediaItems").await? || !has_table(&mut users, "LocalUsersv2").await? {
        return Err(Refusal::Unreadable("not an Emby library".into()));
    }
    Ok(Backup {
        server: Server::Emby,
        accounts: accounts(&mut users, &found.config).await?,
        libraries: libraries(&mut library).await?,
        items: items(&mut library).await?,
        marks: marks(&mut library).await?,
        playlists: playlists(&mut library).await?,
        collections: collections(&mut library).await?,
    })
}

/// What `LocalUsersv2` keeps of an account, as JSON.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Stored {
    name: String,
    id_string: Option<String>,
    #[serde(default)]
    uses_id_for_configuration_path: bool,
}

async fn accounts(users: &mut SqliteConnection, config: &Path) -> Result<Vec<Account>, Refusal> {
    let rows = sqlx::query("SELECT Id, CAST(data AS TEXT) AS data FROM LocalUsersv2 ORDER BY Id")
        .fetch_all(users)
        .await?;
    let mut accounts = Vec::with_capacity(rows.len());
    for row in rows {
        let key: i64 = row.try_get("Id")?;
        let data: String = row.try_get("data")?;
        let stored: Stored =
            serde_json::from_str(&data).map_err(|error| Refusal::Unreadable(format!("account {key}: {error}")))?;
        let folder = match &stored.id_string {
            Some(id) if stored.uses_id_for_configuration_path => config.join(id),
            _ => config.join(&stored.name),
        };
        let policy = read_xml(&folder.join("policy.xml")).await;
        let settings = read_xml(&folder.join("config.xml")).await;
        let policy = policy.as_deref().and_then(|text| roxmltree::Document::parse(text).ok());
        let settings = settings.as_deref().and_then(|text| roxmltree::Document::parse(text).ok());
        let says = |document: &Option<roxmltree::Document>, name: &str| -> Option<String> {
            document.as_ref().and_then(|document| text_of(document, name))
        };
        let yes = |name: &str| says(&policy, name).is_some_and(|value| value.eq_ignore_ascii_case("true"));
        let libraries = if policy.is_none() || yes("EnableAllFolders") {
            None
        } else {
            Some(policy.as_ref().map(|document| strings_of(document, "EnabledFolders")).unwrap_or_default())
        };
        accounts.push(Account {
            key: key.to_string(),
            name: stored.name,
            administrator: yes("IsAdministrator"),
            disabled: yes("IsDisabled"),
            may_delete: yes("EnableContentDeletion"),
            may_download: yes("EnableContentDownloading"),
            libraries,
            audio_language: says(&settings, "AudioLanguagePreference").filter(|value| !value.is_empty()),
            subtitle_language: says(&settings, "SubtitleLanguagePreference").filter(|value| !value.is_empty()),
        });
    }
    Ok(accounts)
}

async fn read_xml(path: &Path) -> Option<String> {
    tokio::fs::read_to_string(path).await.ok()
}

/// The text of the first element of that name.
fn text_of(document: &roxmltree::Document, name: &str) -> Option<String> {
    document
        .descendants()
        .find(|node| node.has_tag_name(name))
        .and_then(|node| node.text())
        .map(|text| text.trim().to_string())
}

/// The strings listed under the first element of that name, as GUIDs.
fn strings_of(document: &roxmltree::Document, name: &str) -> Vec<String> {
    document
        .descendants()
        .find(|node| node.has_tag_name(name))
        .map(|list| {
            list.children()
                .filter(|node| node.has_tag_name("string"))
                .filter_map(|node| node.text().and_then(guid::from_text))
                .collect()
        })
        .unwrap_or_default()
}

/// What Emby keeps of a library's settings, as JSON.
#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct Options {
    #[serde(default)]
    path_infos: Vec<PathInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PathInfo {
    path: Option<String>,
}

async fn libraries(library: &mut SqliteConnection) -> Result<Vec<Library>, Refusal> {
    let rows = sqlx::query(
        "SELECT m.Id, m.guid, m.Name, e.Value
           FROM MediaItems m
           LEFT JOIN ItemExtradata e ON e.ItemId = m.Id
                AND e.ExtradataTypeId = (SELECT ExtradataTypeId FROM ItemExtradataTypes WHERE Name = 'LibraryOptions')
          WHERE m.type = ?
          ORDER BY m.Id",
    )
    .bind(LIBRARY)
    .fetch_all(&mut *library)
    .await?;
    let linked = sqlx::query(
        "SELECT l.ItemId, f.Path
           FROM ItemLinks2 l JOIN MediaItems f ON f.Id = l.LinkedId
          WHERE l.Type = ? AND f.Path IS NOT NULL
          ORDER BY l.ItemId, l.LinkOrder",
    )
    .bind(FOLDERS_LINK)
    .fetch_all(&mut *library)
    .await?;
    let mut linked_folders: HashMap<i64, Vec<String>> = HashMap::new();
    for row in linked {
        linked_folders.entry(row.try_get("ItemId")?).or_default().push(row.try_get("Path")?);
    }
    let mut libraries = Vec::with_capacity(rows.len());
    for row in rows {
        let id: i64 = row.try_get("Id")?;
        let Some(key) = row.try_get::<Option<Vec<u8>>, _>("guid")?.as_deref().and_then(guid::from_blob) else {
            continue;
        };
        let options: Options = row
            .try_get::<Option<String>, _>("Value")?
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default();
        let mut folders: Vec<String> = options.path_infos.into_iter().filter_map(|info| info.path).collect();
        if folders.is_empty() {
            folders = linked_folders.remove(&id).unwrap_or_default();
        }
        libraries.push(Library { key, name: row.try_get::<Option<String>, _>("Name")?.unwrap_or_default(), folders });
    }
    Ok(libraries)
}

fn kind_of(number: i64) -> Option<Kind> {
    Some(match number {
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

async fn items(library: &mut SqliteConnection) -> Result<Vec<Item>, Refusal> {
    let artists = artists(library).await?;
    let rows = sqlx::query(
        "SELECT Id, type, Name, ProductionYear, Path, ProviderIds, IndexNumber, ParentIndexNumber, SeriesId, AlbumId
           FROM MediaItems
          WHERE type IN (?, ?, ?, ?, ?, ?, ?, ?)
          ORDER BY Id",
    )
    .bind(MOVIE)
    .bind(SERIES)
    .bind(SEASON)
    .bind(EPISODE)
    .bind(VIDEO)
    .bind(SONG)
    .bind(ALBUM)
    .bind(ARTIST)
    .fetch_all(&mut *library)
    .await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let id: i64 = row.try_get("Id")?;
        let Some(kind) = kind_of(row.try_get("type")?) else {
            continue;
        };
        let number = |column: &str| -> Result<Option<i32>, Refusal> {
            Ok(row.try_get::<Option<i64>, _>(column)?.and_then(|value| i32::try_from(value).ok()))
        };
        let parent = match kind {
            Kind::Episode | Kind::Season => row.try_get::<Option<i64>, _>("SeriesId")?,
            Kind::Song => row.try_get::<Option<i64>, _>("AlbumId")?,
            _ => None,
        };
        let (season, episode) = match kind {
            Kind::Episode => (number("ParentIndexNumber")?, number("IndexNumber")?),
            Kind::Season => (number("IndexNumber")?, None),
            _ => (None, None),
        };
        items.push(Item {
            key: id.to_string(),
            kind,
            name: row.try_get::<Option<String>, _>("Name")?.unwrap_or_default(),
            year: number("ProductionYear")?,
            path: row.try_get("Path")?,
            ids: provider::ids(row.try_get::<Option<String>, _>("ProviderIds")?.as_deref()),
            season,
            episode,
            parent: parent.map(|parent| parent.to_string()),
            artists: artists.get(&id).cloned().unwrap_or_default(),
        });
    }
    Ok(items)
}

/// The artists each song and album is filed under: the ones its album is
/// credited to, or failing those every one that plays on it.
async fn artists(library: &mut SqliteConnection) -> Result<HashMap<i64, Vec<String>>, Refusal> {
    let rows = sqlx::query(
        "SELECT l.ItemId, l.Type, a.Name
           FROM ItemLinks2 l JOIN MediaItems a ON a.Id = l.LinkedId
          WHERE l.Type IN (?, ?) AND a.type = ? AND a.Name IS NOT NULL
          ORDER BY l.ItemId, l.Type DESC, l.LinkOrder",
    )
    .bind(ARTISTS_LINK)
    .bind(ALBUM_ARTISTS_LINK)
    .bind(ARTIST)
    .fetch_all(library)
    .await?;
    let mut artists: HashMap<i64, (i64, Vec<String>)> = HashMap::new();
    for row in rows {
        let item: i64 = row.try_get("ItemId")?;
        let link: i64 = row.try_get("Type")?;
        let name: String = row.try_get("Name")?;
        let (kept_link, names) = artists.entry(item).or_insert((link, Vec::new()));
        // The album artists come first; the others only when there are none.
        if *kept_link == link {
            names.push(name);
        }
    }
    Ok(artists.into_iter().map(|(item, (_, names))| (item, names)).collect())
}

async fn marks(library: &mut SqliteConnection) -> Result<Vec<Mark>, Refusal> {
    let rows = sqlx::query(
        "SELECT u.userId, m.Id, u.played, u.playCount, u.isFavorite, u.playbackPositionTicks, u.LastPlayedDateInt
           FROM UserDatas u JOIN MediaItems m ON m.UserDataKeyId = u.UserDataKeyId
          WHERE m.type IN (?, ?, ?, ?, ?, ?, ?, ?)
            AND (u.played <> 0 OR u.playCount > 0 OR u.isFavorite <> 0 OR u.playbackPositionTicks > 0)
          ORDER BY u.userId, m.Id",
    )
    .bind(MOVIE)
    .bind(SERIES)
    .bind(SEASON)
    .bind(EPISODE)
    .bind(VIDEO)
    .bind(SONG)
    .bind(ALBUM)
    .bind(ARTIST)
    .fetch_all(library)
    .await?;
    let mut marks = Vec::with_capacity(rows.len());
    for row in rows {
        let played_at: Option<i64> = row.try_get("LastPlayedDateInt")?;
        marks.push(Mark {
            account: row.try_get::<i64, _>("userId")?.to_string(),
            item: row.try_get::<i64, _>("Id")?.to_string(),
            played: row.try_get::<i64, _>("played")? != 0,
            play_count: u32::try_from(row.try_get::<i64, _>("playCount")?).unwrap_or(0),
            position_ms: row.try_get::<i64, _>("playbackPositionTicks")?.max(0) / TICKS_A_MILLISECOND,
            last_played: played_at
                .filter(|seconds| *seconds > 0)
                .and_then(|seconds| OffsetDateTime::from_unix_timestamp(seconds).ok()),
            favorite: row.try_get::<i64, _>("isFavorite")? != 0,
        });
    }
    Ok(marks)
}

/// The items of every list, a playlist or a collection, in their order.
async fn listed(library: &mut SqliteConnection) -> Result<HashMap<i64, Vec<String>>, Refusal> {
    let rows = sqlx::query("SELECT ListId, ListItemId FROM ListItems ORDER BY ListId, ListItemOrder, ListItemEntryId")
        .fetch_all(library)
        .await?;
    let mut listed: HashMap<i64, Vec<String>> = HashMap::new();
    for row in rows {
        listed
            .entry(row.try_get("ListId")?)
            .or_default()
            .push(row.try_get::<i64, _>("ListItemId")?.to_string());
    }
    Ok(listed)
}

async fn playlists(library: &mut SqliteConnection) -> Result<Vec<Playlist>, Refusal> {
    let mut listed = listed(library).await?;
    let mut owners: HashMap<i64, String> = HashMap::new();
    // The share that gives the most is the owner's.
    for row in sqlx::query("SELECT ItemId, UserId FROM UserItemShares ORDER BY ItemId, ShareLevel DESC")
        .fetch_all(&mut *library)
        .await?
    {
        let item: i64 = row.try_get("ItemId")?;
        let user: i64 = row.try_get("UserId")?;
        owners.entry(item).or_insert_with(|| user.to_string());
    }
    let rows = sqlx::query("SELECT Id, Name FROM MediaItems WHERE type = ? ORDER BY Id")
        .bind(PLAYLIST)
        .fetch_all(&mut *library)
        .await?;
    let mut playlists = Vec::with_capacity(rows.len());
    for row in rows {
        let id: i64 = row.try_get("Id")?;
        playlists.push(Playlist {
            name: row.try_get::<Option<String>, _>("Name")?.unwrap_or_default(),
            owner: owners.remove(&id),
            items: listed.remove(&id).unwrap_or_default(),
        });
    }
    Ok(playlists)
}

/// Every list that is not a playlist is a collection. One Emby made itself
/// from the provider is in `ImportedCollections`, or names the provider's
/// collection; one made by hand does neither.
async fn collections(library: &mut SqliteConnection) -> Result<Vec<Collection>, Refusal> {
    let mut listed = listed(library).await?;
    let imported: HashSet<i64> = if has_table(library, "ImportedCollections").await? {
        sqlx::query("SELECT DISTINCT ItemId FROM ImportedCollections WHERE ItemId IS NOT NULL")
            .fetch_all(&mut *library)
            .await?
            .iter()
            .map(|row| row.try_get("ItemId"))
            .collect::<Result<_, _>>()?
    } else {
        HashSet::new()
    };
    let rows = sqlx::query(
        "SELECT DISTINCT m.Id, m.Name, m.ProviderIds
           FROM ListItems l JOIN MediaItems m ON m.Id = l.ListId
          WHERE m.type <> ?
          ORDER BY m.Id",
    )
    .bind(PLAYLIST)
    .fetch_all(&mut *library)
    .await?;
    let mut collections = Vec::with_capacity(rows.len());
    for row in rows {
        let id: i64 = row.try_get("Id")?;
        let from_provider = provider::ids(row.try_get::<Option<String>, _>("ProviderIds")?.as_deref())
            .iter()
            .any(|(provider, _)| provider == "tmdb");
        collections.push(Collection {
            name: row.try_get::<Option<String>, _>("Name")?.unwrap_or_default(),
            items: listed.remove(&id).unwrap_or_default(),
            automatic: Some(imported.contains(&id) || from_provider),
        });
    }
    Ok(collections)
}
