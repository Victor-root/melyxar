//! Jellyfin's own databases: `jellyfin.db` alone from 10.11, and before that
//! `library.db` beside it for the items and what each account did with them.

use std::collections::HashMap;
use std::path::Path;

use sqlx::Row;
use sqlx::sqlite::{SqliteConnection, SqliteRow};

use super::{KINDS, LISTS, Member, OTHERS, Played, Setting, Stored, Tables, User, date_of};
use crate::copy::{has_column, has_table, opened};
use crate::{Refusal, Server, guid, provider};

pub(super) async fn tables(data: &Path, work: &Path) -> Result<Tables, Refusal> {
    let mut jellyfin = opened(&data.join("jellyfin.db"), work).await?;
    if !has_table(&mut jellyfin, "Users").await? {
        return Err(Refusal::Unreadable("not a Jellyfin database".into()));
    }
    if has_table(&mut jellyfin, "BaseItems").await? {
        let members = if has_table(&mut jellyfin, "LinkedChildren").await? {
            Some(members(&mut jellyfin).await?)
        } else {
            None
        };
        return Ok(Tables {
            users: users(&mut jellyfin).await?,
            permissions: permissions(&mut jellyfin).await?,
            preferences: preferences(&mut jellyfin).await?,
            items: items(&mut jellyfin).await?,
            ids: ids(&mut jellyfin).await?,
            plays: plays(&mut jellyfin).await?,
            members,
        });
    }
    let library = data.join("library.db");
    if !library.is_file() {
        return Err(Refusal::Unreadable("no library.db beside jellyfin.db".into()));
    }
    let mut library = opened(&library, work).await?;
    if !has_table(&mut library, "TypedBaseItems").await? {
        return Err(Refusal::Unreadable("not a Jellyfin library".into()));
    }
    // The columns 10.9.11 added, which Jellyfin itself looks for before
    // taking up a library.db.
    if !has_column(&mut library, "TypedBaseItems", "LUFS").await?
        || !has_column(&mut library, "TypedBaseItems", "NormalizationGain").await?
        || !has_column(&mut library, "mediastreams", "DvVersionMajor").await?
    {
        return Err(Refusal::TooOld { server: Server::Jellyfin });
    }
    let users = users(&mut jellyfin).await?;
    let (items, ids) = typed_items(&mut library).await?;
    let plays = typed_plays(&mut library, &users).await?;
    Ok(Tables {
        users,
        permissions: permissions(&mut jellyfin).await?,
        preferences: preferences(&mut jellyfin).await?,
        items,
        ids,
        plays,
        members: None,
    })
}

/// A GUID written as text, whichever way.
fn text_key(row: &SqliteRow, column: &str) -> Result<Option<String>, Refusal> {
    Ok(row.try_get::<Option<String>, _>(column)?.as_deref().and_then(guid::from_text))
}

/// A GUID written as .NET bytes, as `library.db` keeps them.
fn blob_key(row: &SqliteRow, column: &str) -> Result<Option<String>, Refusal> {
    Ok(row.try_get::<Option<Vec<u8>>, _>(column)?.as_deref().and_then(guid::from_blob))
}

fn needed(key: Option<String>, column: &str) -> Result<String, Refusal> {
    key.ok_or_else(|| Refusal::Unreadable(format!("{column} is not a GUID")))
}

fn number(row: &SqliteRow, column: &str) -> Result<Option<i32>, Refusal> {
    Ok(row.try_get::<Option<i64>, _>(column)?.and_then(|value| i32::try_from(value).ok()))
}

async fn users(jellyfin: &mut SqliteConnection) -> Result<Vec<User>, Refusal> {
    let rows = sqlx::query(
        "SELECT Id, Username, InternalId, AudioLanguagePreference, SubtitleLanguagePreference FROM Users",
    )
    .fetch_all(jellyfin)
    .await?;
    rows.iter()
        .map(|row| {
            Ok(User {
                id: needed(text_key(row, "Id")?, "Users.Id")?,
                username: row.try_get("Username")?,
                internal_id: row.try_get("InternalId")?,
                audio_language_preference: row.try_get("AudioLanguagePreference")?,
                subtitle_language_preference: row.try_get("SubtitleLanguagePreference")?,
            })
        })
        .collect()
}

async fn permissions(jellyfin: &mut SqliteConnection) -> Result<Vec<Setting<bool>>, Refusal> {
    let rows = sqlx::query("SELECT UserId, Kind, Value FROM Permissions").fetch_all(jellyfin).await?;
    rows.iter()
        .map(|row| {
            Ok(Setting {
                user_id: text_key(row, "UserId")?,
                kind: row.try_get("Kind")?,
                value: row.try_get::<i64, _>("Value")? != 0,
            })
        })
        .collect()
}

async fn preferences(jellyfin: &mut SqliteConnection) -> Result<Vec<Setting<String>>, Refusal> {
    let rows = sqlx::query("SELECT UserId, Kind, Value FROM Preferences").fetch_all(jellyfin).await?;
    rows.iter()
        .map(|row| {
            Ok(Setting { user_id: text_key(row, "UserId")?, kind: row.try_get("Kind")?, value: row.try_get("Value")? })
        })
        .collect()
}

/// An item, its identifiers read by `key` as each era writes them.
fn stored(row: &SqliteRow, key: fn(&SqliteRow, &str) -> Result<Option<String>, Refusal>) -> Result<Stored, Refusal> {
    Ok(Stored {
        id: needed(key(row, "Id")?, "item Id")?,
        kind: row.try_get("Type")?,
        name: row.try_get("Name")?,
        path: row.try_get("Path")?,
        production_year: number(row, "ProductionYear")?,
        index_number: number(row, "IndexNumber")?,
        parent_index_number: number(row, "ParentIndexNumber")?,
        series_id: key(row, "SeriesId")?,
        parent_id: key(row, "ParentId")?,
        artists: row.try_get("Artists")?,
        album_artists: row.try_get("AlbumArtists")?,
        data: row.try_get("Data")?,
    })
}

async fn items(jellyfin: &mut SqliteConnection) -> Result<Vec<Stored>, Refusal> {
    let mut query = sqlx::query(
        "SELECT Id, Type, Name, Path, ProductionYear, IndexNumber, ParentIndexNumber, SeriesId, ParentId,
                Artists, AlbumArtists, CASE WHEN Type IN (?, ?) THEN Data END AS Data
           FROM BaseItems
          WHERE Type IN (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    );
    for kind in LISTS.iter().chain(&KINDS).chain(&OTHERS) {
        query = query.bind(*kind);
    }
    query.fetch_all(jellyfin).await?.iter().map(|row| stored(row, text_key)).collect()
}

async fn ids(jellyfin: &mut SqliteConnection) -> Result<HashMap<String, Vec<(String, String)>>, Refusal> {
    let mut query = sqlx::query(
        "SELECT p.ItemId, p.ProviderId, p.ProviderValue
           FROM BaseItemProviders p JOIN BaseItems b ON b.Id = p.ItemId
          WHERE b.Type IN (?, ?, ?, ?, ?, ?, ?, ?)",
    );
    for kind in KINDS {
        query = query.bind(kind);
    }
    let mut ids: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for row in query.fetch_all(jellyfin).await? {
        let item = needed(text_key(&row, "ItemId")?, "BaseItemProviders.ItemId")?;
        let provider: String = row.try_get("ProviderId")?;
        let value: String = row.try_get("ProviderValue")?;
        ids.entry(item).or_default().extend(provider::id(&provider, &value));
    }
    Ok(ids)
}

async fn plays(jellyfin: &mut SqliteConnection) -> Result<Vec<Played>, Refusal> {
    let rows = sqlx::query(
        "SELECT ItemId, UserId, Played, PlayCount, IsFavorite, PlaybackPositionTicks, LastPlayedDate FROM UserData",
    )
    .fetch_all(jellyfin)
    .await?;
    rows.iter()
        .map(|row| {
            Ok(Played {
                item_id: needed(text_key(row, "ItemId")?, "UserData.ItemId")?,
                user_id: needed(text_key(row, "UserId")?, "UserData.UserId")?,
                played: row.try_get::<i64, _>("Played")? != 0,
                play_count: row.try_get("PlayCount")?,
                is_favorite: row.try_get::<i64, _>("IsFavorite")? != 0,
                playback_position_ticks: row.try_get("PlaybackPositionTicks")?,
                last_played_date: row.try_get::<Option<String>, _>("LastPlayedDate")?.as_deref().and_then(date_of),
            })
        })
        .collect()
}

async fn members(jellyfin: &mut SqliteConnection) -> Result<Vec<Member>, Refusal> {
    let rows = sqlx::query("SELECT ParentId, ChildId, SortOrder FROM LinkedChildren").fetch_all(jellyfin).await?;
    rows.iter()
        .map(|row| {
            Ok(Member {
                parent_id: needed(text_key(row, "ParentId")?, "LinkedChildren.ParentId")?,
                child_id: needed(text_key(row, "ChildId")?, "LinkedChildren.ChildId")?,
                sort_order: row.try_get("SortOrder")?,
            })
        })
        .collect()
}

/// The items of `library.db`, up to 10.10, with their identifiers written
/// together as `Tmdb=603|Imdb=tt0133093`.
async fn typed_items(
    library: &mut SqliteConnection,
) -> Result<(Vec<Stored>, HashMap<String, Vec<(String, String)>>), Refusal> {
    let mut query = sqlx::query(
        "SELECT guid AS Id, type AS Type, Name, Path, ProductionYear, IndexNumber, ParentIndexNumber, SeriesId,
                ParentId, Artists, AlbumArtists, ProviderIds,
                CASE WHEN type IN (?, ?) THEN CAST(data AS TEXT) END AS Data
           FROM TypedBaseItems
          WHERE type IN (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    );
    for kind in LISTS.iter().chain(&KINDS).chain(&OTHERS) {
        query = query.bind(*kind);
    }
    let mut items = Vec::new();
    let mut ids = HashMap::new();
    for row in query.fetch_all(library).await? {
        let item = stored(&row, blob_key)?;
        ids.insert(item.id.clone(), provider::ids(row.try_get::<Option<String>, _>("ProviderIds")?.as_deref()));
        items.push(item);
    }
    Ok((items, ids))
}

/// What each account did with each item, up to 10.10: kept under the key
/// each item reads, and naming the account by its number.
async fn typed_plays(library: &mut SqliteConnection, users: &[User]) -> Result<Vec<Played>, Refusal> {
    let accounts: HashMap<i64, &str> = users.iter().map(|user| (user.internal_id, user.id.as_str())).collect();
    let mut query = sqlx::query(
        "SELECT u.userId, t.guid, u.played, u.playCount, u.isFavorite, u.playbackPositionTicks, u.lastPlayedDate
           FROM UserDatas u JOIN TypedBaseItems t ON t.UserDataKey = u.key
          WHERE t.type IN (?, ?, ?, ?, ?, ?, ?, ?)",
    );
    for kind in KINDS {
        query = query.bind(kind);
    }
    let mut plays = Vec::new();
    for row in query.fetch_all(library).await? {
        let Some(account) = accounts.get(&row.try_get::<i64, _>("userId")?) else {
            continue;
        };
        plays.push(Played {
            item_id: needed(blob_key(&row, "guid")?, "TypedBaseItems.guid")?,
            user_id: account.to_string(),
            played: row.try_get::<i64, _>("played")? != 0,
            play_count: row.try_get("playCount")?,
            is_favorite: row.try_get::<i64, _>("isFavorite")? != 0,
            playback_position_ticks: row.try_get("playbackPositionTicks")?,
            last_played_date: row.try_get::<Option<String>, _>("lastPlayedDate")?.as_deref().and_then(date_of),
        });
    }
    Ok(plays)
}
