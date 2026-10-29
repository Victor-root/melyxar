//! Jellyfin data folders made up here, one for each way its files have been
//! laid out since 10.9.11, and the backup it makes of itself, unzipped. All
//! hold the same accounts, items and plays, written out as each version
//! writes them, and all read back to the same backup.

mod common;

use std::path::Path;

use common::{database, opened};
use melyxar_import::{Account, Backup, Collection, Item, Kind, Library, Mark, Playlist, Refusal, Server, read};
use serde_json::{Value, json};
use sqlx::sqlite::SqliteConnection;
use sqlx::{Connection, Executor};
use time::macros::datetime;
use uuid::Uuid;

const MOVIE: &str = "MediaBrowser.Controller.Entities.Movies.Movie";
const SERIES: &str = "MediaBrowser.Controller.Entities.TV.Series";
const SEASON: &str = "MediaBrowser.Controller.Entities.TV.Season";
const EPISODE: &str = "MediaBrowser.Controller.Entities.TV.Episode";
const VIDEO: &str = "MediaBrowser.Controller.Entities.Video";
const SONG: &str = "MediaBrowser.Controller.Entities.Audio.Audio";
const ALBUM: &str = "MediaBrowser.Controller.Entities.Audio.MusicAlbum";
const ARTIST: &str = "MediaBrowser.Controller.Entities.Audio.MusicArtist";
const PERSON: &str = "MediaBrowser.Controller.Entities.Person";
const LIBRARY: &str = "MediaBrowser.Controller.Entities.CollectionFolder";
const PLAYLIST: &str = "MediaBrowser.Controller.Playlists.Playlist";
const COLLECTION: &str = "MediaBrowser.Controller.Entities.Movies.BoxSet";

const ADA: &str = "ADA00000-0000-0000-0000-000000000001";
const BEN: &str = "BE000000-0000-0000-0000-000000000002";
const FILMS: &str = "F0000000-0000-0000-0000-000000000001";
const HARBOUR: &str = "10000000-0000-0000-0000-000000000001";
const HARBOUR_4K: &str = "10000000-0000-0000-0000-000000000002";
const VALLEY: &str = "20000000-0000-0000-0000-000000000001";
const SEASON_TWO: &str = "20000000-0000-0000-0000-000000000002";
const SNOWFALL: &str = "20000000-0000-0000-0000-000000000003";
const BIRDS: &str = "30000000-0000-0000-0000-000000000001";
const NORTH_WIND: &str = "30000000-0000-0000-0000-000000000002";
const FIRST_LIGHT: &str = "30000000-0000-0000-0000-000000000003";
const PICNIC: &str = "40000000-0000-0000-0000-000000000001";
const SOMEBODY: &str = "50000000-0000-0000-0000-000000000001";
const EVENING: &str = "60000000-0000-0000-0000-000000000001";
const RAINY_DAYS: &str = "70000000-0000-0000-0000-000000000001";
/// Where Jellyfin puts what accounts did with items it no longer has.
const PLACEHOLDER: &str = "00000000-0000-0000-0000-000000000001";

const SNOWFALL_FILE: &str = "/disks/one/Series/Quiet Valley/Season 02/S02E05.mkv";
const FIRST_LIGHT_FILE: &str = "/disks/one/Music/The Paper Birds/North Wind/01 First Light.flac";
const NORTH_WIND_MBID: &str = "0f0e0d0c-1111-2222-3333-444455556666";

/// An account, its rights and its preferences.
struct Someone {
    id: &'static str,
    name: &'static str,
    internal_id: i64,
    audio: Option<&'static str>,
    subtitle: Option<&'static str>,
    rights: &'static [(i64, bool)],
    preferences: &'static [(i64, &'static str)],
}

const PEOPLE: [Someone; 2] = [
    Someone {
        id: ADA,
        name: "Ada",
        internal_id: 1,
        audio: Some("fre"),
        subtitle: Some(""),
        rights: &[(0, true), (2, false), (7, true), (10, true), (11, false), (16, true)],
        preferences: &[(5, "")],
    },
    Someone {
        id: BEN,
        name: "Ben",
        internal_id: 2,
        audio: None,
        subtitle: Some("eng"),
        rights: &[(0, false), (2, false), (7, true), (10, false), (11, true), (16, false)],
        preferences: &[(0, "Horror"), (5, "f0000000-0000-0000-0000-000000000001")],
    },
];

/// An item, with what each version keeps of it.
struct Made {
    id: &'static str,
    kind: &'static str,
    name: &'static str,
    path: Option<&'static str>,
    parent: Option<&'static str>,
    series: Option<&'static str>,
    index: Option<i64>,
    parent_index: Option<i64>,
    year: Option<i64>,
    artists: Option<&'static str>,
    album_artists: Option<&'static str>,
    /// The key it reads what accounts did with it under, up to 10.10.
    user_data_key: &'static str,
    ids: &'static [(&'static str, &'static str)],
}

const NOTHING: Made = Made {
    id: "",
    kind: "",
    name: "",
    path: None,
    parent: None,
    series: None,
    index: None,
    parent_index: None,
    year: None,
    artists: None,
    album_artists: None,
    user_data_key: "",
    ids: &[],
};

/// A library, a film in two copies, a series with a season and an episode,
/// an artist with an album and a song, a home video, and a person, who is
/// not taken up.
const ITEMS: [Made; 11] = [
    Made {
        id: FILMS,
        kind: LIBRARY,
        name: "Films",
        path: Some("/srv/jellyfin/root/default/Films"),
        user_data_key: "f0000000-0000-0000-0000-000000000001",
        ..NOTHING
    },
    Made {
        id: HARBOUR,
        kind: MOVIE,
        name: "Harbour Lights",
        path: Some("/disks/one/Films/Harbour Lights (2011).mkv"),
        parent: Some(FILMS),
        year: Some(2011),
        user_data_key: "1001",
        ids: &[("Tmdb", "1001"), ("Imdb", "tt0001001")],
        ..NOTHING
    },
    Made {
        id: HARBOUR_4K,
        kind: MOVIE,
        name: "Harbour Lights",
        path: Some("/disks/two/Films/Harbour Lights (2011) 4K.mkv"),
        parent: Some(FILMS),
        year: Some(2011),
        user_data_key: "1001",
        ids: &[("Tmdb", "1001")],
        ..NOTHING
    },
    Made {
        id: VALLEY,
        kind: SERIES,
        name: "Quiet Valley",
        path: Some("/disks/one/Series/Quiet Valley"),
        year: Some(2019),
        user_data_key: "2002",
        ids: &[("Tvdb", "2002"), ("Tmdb", "3003")],
        ..NOTHING
    },
    Made {
        id: SEASON_TWO,
        kind: SEASON,
        name: "Season 2",
        path: Some("/disks/one/Series/Quiet Valley/Season 02"),
        parent: Some(VALLEY),
        series: Some(VALLEY),
        index: Some(2),
        user_data_key: "2002002",
        ..NOTHING
    },
    Made {
        id: SNOWFALL,
        kind: EPISODE,
        name: "Snowfall",
        path: Some(SNOWFALL_FILE),
        parent: Some(SEASON_TWO),
        series: Some(VALLEY),
        index: Some(5),
        parent_index: Some(2),
        user_data_key: "2002002005",
        ids: &[("Tvdb", "4004")],
        ..NOTHING
    },
    Made {
        id: BIRDS,
        kind: ARTIST,
        name: "The Paper Birds",
        path: Some("/disks/one/Music/The Paper Birds"),
        user_data_key: "Artist-The Paper Birds",
        ..NOTHING
    },
    Made {
        id: NORTH_WIND,
        kind: ALBUM,
        name: "North Wind",
        path: Some("/disks/one/Music/The Paper Birds/North Wind"),
        parent: Some(BIRDS),
        year: Some(2015),
        artists: Some("The Paper Birds"),
        album_artists: Some(""),
        user_data_key: "MusicAlbum-Musicbrainz-0f0e0d0c-1111-2222-3333-444455556666",
        ids: &[("MusicBrainzAlbum", NORTH_WIND_MBID)],
        ..NOTHING
    },
    Made {
        id: FIRST_LIGHT,
        kind: SONG,
        name: "First Light",
        path: Some(FIRST_LIGHT_FILE),
        parent: Some(NORTH_WIND),
        index: Some(1),
        artists: Some("The Paper Birds|A Guest"),
        album_artists: Some("The Paper Birds"),
        user_data_key: "The Paper Birds-North Wind-0001First Light",
        ..NOTHING
    },
    Made {
        id: PICNIC,
        kind: VIDEO,
        name: "Picnic",
        path: Some("/disks/one/Home/Picnic.mp4"),
        user_data_key: "40000000-0000-0000-0000-000000000001",
        ..NOTHING
    },
    Made {
        id: SOMEBODY,
        kind: PERSON,
        name: "Somebody",
        path: Some("%MetadataPath%/People/S/Somebody"),
        user_data_key: "Person-Somebody",
        ids: &[("Tmdb", "9")],
        ..NOTHING
    },
];

/// A playlist and a collection, whose members are written inside them until
/// 12.
const LISTS: [Made; 2] = [
    Made {
        id: EVENING,
        kind: PLAYLIST,
        name: "Evening",
        path: Some("%AppDataPath%/playlists/Evening"),
        user_data_key: "60000000-0000-0000-0000-000000000001",
        ..NOTHING
    },
    Made {
        id: RAINY_DAYS,
        kind: COLLECTION,
        name: "Rainy days",
        path: Some("%AppDataPath%/collections/Rainy days [boxset]"),
        user_data_key: "70000000-0000-0000-0000-000000000001",
        ..NOTHING
    },
];

/// The list's JSON, with its members inside it or, from 12, without.
fn list_data(id: &str, members_inside: bool) -> String {
    let mut data = if id == EVENING {
        json!({
            "OwnerUserId": "ada00000000000000000000000000001",
            "OpenAccess": false,
            "Shares": [{ "UserId": "ada00000-0000-0000-0000-000000000001", "CanEdit": true }],
            "PlaylistMediaType": "Video",
            "LinkedChildren": [
                // The film moved since it was added: the item it was found
                // to be is what counts.
                { "Path": "/disks/gone/Harbour Lights.mkv", "Type": "Manual", "ItemId": "10000000000000000000000000000001" },
                { "Path": FIRST_LIGHT_FILE, "Type": "Manual" },
                { "Path": "/disks/one/Films/Long Gone.mkv", "Type": "Manual" },
                { "Type": "Manual", "LibraryItemId": "40000000000000000000000000000001" },
            ],
        })
    } else {
        json!({
            "DisplayOrder": "PremiereDate",
            "LinkedChildren": [{ "Path": SNOWFALL_FILE, "Type": "Manual" }],
        })
    };
    if !members_inside {
        data.as_object_mut().unwrap().remove("LinkedChildren");
    }
    data.to_string()
}

/// The members of the lists in their table, from 12, with a film's other
/// copy, which is no list's.
const LINKED: [(&str, i64, &str, i64); 5] = [
    (EVENING, 0, HARBOUR, 0),
    (EVENING, 1, FIRST_LIGHT, 0),
    (EVENING, 2, PICNIC, 0),
    (RAINY_DAYS, 0, SNOWFALL, 0),
    (HARBOUR, 0, HARBOUR_4K, 2),
];

/// What an account did with an item, under one of its keys.
struct Play {
    item: &'static str,
    user: &'static str,
    key: &'static str,
    played: bool,
    count: i64,
    favorite: bool,
    ticks: i64,
    at: Option<&'static str>,
}

/// Ada watched the film long ago under its IMDb key, then again under its
/// TMDb key, and left the episode partway; Ben listened to the song and
/// likes the album. Nothing happened with the home video, the person is not
/// taken up, and the last row is of an item Jellyfin no longer has.
const PLAYS: [Play; 10] = [
    Play { item: HARBOUR, user: ADA, key: "tt0001001", played: true, count: 5, favorite: false, ticks: 0, at: Some("2020-01-01 00:00:00") },
    Play { item: HARBOUR, user: ADA, key: "1001", played: true, count: 2, favorite: true, ticks: 0, at: Some("2023-11-14 22:13:20") },
    Play { item: HARBOUR_4K, user: ADA, key: "1001", played: true, count: 2, favorite: true, ticks: 0, at: Some("2023-11-14 22:13:20") },
    Play { item: HARBOUR_4K, user: ADA, key: "tt0001001", played: true, count: 5, favorite: false, ticks: 0, at: Some("2020-01-01 00:00:00") },
    Play { item: SNOWFALL, user: ADA, key: "2002002005", played: false, count: 0, favorite: false, ticks: 12_345_670_000, at: Some("2023-11-14 22:15:00.5") },
    Play { item: FIRST_LIGHT, user: BEN, key: "The Paper Birds-North Wind-0001First Light", played: true, count: 7, favorite: false, ticks: 0, at: None },
    Play { item: NORTH_WIND, user: BEN, key: "MusicAlbum-Musicbrainz-0f0e0d0c-1111-2222-3333-444455556666", played: false, count: 0, favorite: true, ticks: 0, at: None },
    Play { item: PICNIC, user: ADA, key: "40000000-0000-0000-0000-000000000001", played: false, count: 0, favorite: false, ticks: 0, at: None },
    Play { item: SOMEBODY, user: BEN, key: "Person-Somebody", played: true, count: 1, favorite: false, ticks: 0, at: None },
    Play { item: PLACEHOLDER, user: ADA, key: "Person-Gone", played: true, count: 3, favorite: true, ticks: 0, at: Some("2024-05-01 10:00:00") },
];

/// The accounts' tables of `jellyfin.db`, the same from 10.9 on as far as
/// they are read.
const ACCOUNTS_SCHEMA: &str = r#"
CREATE TABLE "Users" ("Id" TEXT NOT NULL CONSTRAINT "PK_Users" PRIMARY KEY, "Username" TEXT NOT NULL COLLATE NOCASE,
    "Password" TEXT NULL, "MustUpdatePassword" INTEGER NOT NULL, "AudioLanguagePreference" TEXT NULL,
    "AuthenticationProviderId" TEXT NOT NULL, "PasswordResetProviderId" TEXT NOT NULL,
    "InvalidLoginAttemptCount" INTEGER NOT NULL, "SubtitleMode" INTEGER NOT NULL, "SubtitleLanguagePreference" TEXT NULL,
    "InternalId" INTEGER NOT NULL, "RowVersion" INTEGER NOT NULL);
CREATE TABLE "Permissions" ("Id" INTEGER NOT NULL CONSTRAINT "PK_Permissions" PRIMARY KEY AUTOINCREMENT,
    "Kind" INTEGER NOT NULL, "Value" INTEGER NOT NULL, "RowVersion" INTEGER NOT NULL, "UserId" TEXT NULL,
    CONSTRAINT "FK_Permissions_Users_UserId" FOREIGN KEY ("UserId") REFERENCES "Users" ("Id") ON DELETE CASCADE);
CREATE UNIQUE INDEX "IX_Permissions_UserId_Kind" ON "Permissions" ("UserId", "Kind");
CREATE TABLE "Preferences" ("Id" INTEGER NOT NULL CONSTRAINT "PK_Preferences" PRIMARY KEY AUTOINCREMENT,
    "Kind" INTEGER NOT NULL, "Value" TEXT NOT NULL, "RowVersion" INTEGER NOT NULL, "UserId" TEXT NULL,
    CONSTRAINT "FK_Preferences_Users_UserId" FOREIGN KEY ("UserId") REFERENCES "Users" ("Id") ON DELETE CASCADE);
CREATE UNIQUE INDEX "IX_Preferences_UserId_Kind" ON "Preferences" ("UserId", "Kind");
"#;

/// `library.db` as 10.9.11 to 10.10 leave it.
const LIBRARY_SCHEMA: &str = "
CREATE TABLE TypedBaseItems (guid GUID primary key NOT NULL, type TEXT NOT NULL, data BLOB NULL, ParentId GUID NULL,
    Path TEXT NULL, Name Text, IndexNumber INT, ParentIndexNumber INT, ProductionYear INT, IsFolder BIT, TopParentId Text,
    UserDataKey Text, SeriesId GUID, SeriesName Text, ProviderIds Text, Artists Text, AlbumArtists Text, Album Text,
    IsVirtualItem BIT, LUFS Float, NormalizationGain Float);
CREATE TABLE UserDatas (key nvarchar not null, userId INT not null, rating float null, played bit not null,
    playCount int not null, isFavorite bit not null, playbackPositionTicks bigint not null, lastPlayedDate datetime null,
    AudioStreamIndex INT, SubtitleStreamIndex INT);
CREATE UNIQUE INDEX UserDatasIndex1 on UserDatas (key, userId);
CREATE TABLE mediastreams (ItemId GUID, StreamIndex INT, StreamType TEXT, Codec TEXT, Language TEXT, DvVersionMajor INT,
    PRIMARY KEY (ItemId, StreamIndex));
";

/// The items' tables 10.11 added to `jellyfin.db`.
const ITEMS_SCHEMA: &str = r#"
CREATE TABLE "BaseItems" ("Id" TEXT NOT NULL CONSTRAINT "PK_BaseItems" PRIMARY KEY, "Type" TEXT NOT NULL, "Data" TEXT NULL,
    "Path" TEXT NULL, "ParentId" TEXT NULL, "TopParentId" TEXT NULL, "SeasonId" TEXT NULL, "SeriesId" TEXT NULL,
    "OwnerId" TEXT NULL, "Name" TEXT NULL, "IndexNumber" INTEGER NULL, "ParentIndexNumber" INTEGER NULL,
    "ProductionYear" INTEGER NULL, "Album" TEXT NULL, "Artists" TEXT NULL, "AlbumArtists" TEXT NULL,
    "IsFolder" INTEGER NOT NULL, "IsVirtualItem" INTEGER NOT NULL, "IsLocked" INTEGER NOT NULL);
CREATE TABLE "BaseItemProviders" ("ItemId" TEXT NOT NULL, "ProviderId" TEXT NOT NULL, "ProviderValue" TEXT NOT NULL,
    CONSTRAINT "PK_BaseItemProviders" PRIMARY KEY ("ItemId", "ProviderId"),
    CONSTRAINT "FK_BaseItemProviders_BaseItems_ItemId" FOREIGN KEY ("ItemId") REFERENCES "BaseItems" ("Id") ON DELETE CASCADE);
CREATE TABLE "UserData" ("ItemId" TEXT NOT NULL, "UserId" TEXT NOT NULL, "CustomDataKey" TEXT NOT NULL, "Rating" REAL NULL,
    "PlaybackPositionTicks" INTEGER NOT NULL, "PlayCount" INTEGER NOT NULL, "IsFavorite" INTEGER NOT NULL,
    "LastPlayedDate" TEXT NULL, "Played" INTEGER NOT NULL, "AudioStreamIndex" INTEGER NULL,
    "SubtitleStreamIndex" INTEGER NULL, "Likes" INTEGER NULL, "RetentionDate" TEXT NULL,
    CONSTRAINT "PK_UserData" PRIMARY KEY ("ItemId", "UserId", "CustomDataKey"),
    CONSTRAINT "FK_UserData_BaseItems_ItemId" FOREIGN KEY ("ItemId") REFERENCES "BaseItems" ("Id") ON DELETE CASCADE,
    CONSTRAINT "FK_UserData_Users_UserId" FOREIGN KEY ("UserId") REFERENCES "Users" ("Id") ON DELETE CASCADE);
"#;

/// The table 12 added for the members of lists.
const MEMBERS_SCHEMA: &str = r#"
CREATE TABLE "LinkedChildren" ("ParentId" TEXT NOT NULL, "SortOrder" INTEGER NOT NULL, "ChildId" TEXT NOT NULL,
    "ChildType" INTEGER NOT NULL, CONSTRAINT "PK_LinkedChildren" PRIMARY KEY ("ParentId", "SortOrder"),
    CONSTRAINT "FK_LinkedChildren_BaseItems_ChildId" FOREIGN KEY ("ChildId") REFERENCES "BaseItems" ("Id") ON DELETE CASCADE,
    CONSTRAINT "FK_LinkedChildren_BaseItems_ParentId" FOREIGN KEY ("ParentId") REFERENCES "BaseItems" ("Id") ON DELETE CASCADE);
"#;

/// The thirty-two lower case digits a GUID is read as.
fn key(guid: &str) -> String {
    Uuid::parse_str(guid).unwrap().simple().to_string()
}

/// A GUID as .NET writes it to a blob.
fn blob(guid: &str) -> Vec<u8> {
    Uuid::parse_str(guid).unwrap().to_bytes_le().to_vec()
}

/// The libraries as Jellyfin keeps them: a folder each, holding one
/// `.mblink` file for each folder of media and the library's settings.
fn libraries(root: &Path) {
    let films = root.join("default/Films");
    std::fs::create_dir_all(&films).unwrap();
    std::fs::write(films.join("two.mblink"), "/disks/two/Films\n").unwrap();
    std::fs::write(films.join("one.mblink"), "/disks/one/Films").unwrap();
    std::fs::write(films.join("options.xml"), "<LibraryOptions />").unwrap();
    std::fs::write(films.join("movies.collection"), "").unwrap();
}

async fn run(connection: &mut SqliteConnection, sql: &str) {
    connection.execute(sqlx::raw_sql(sqlx::AssertSqlSafe(sql))).await.unwrap();
}

/// `jellyfin.db` with its accounts, left open for what the version adds.
async fn accounts(path: &Path) -> SqliteConnection {
    let mut connection = opened(path).await;
    run(&mut connection, ACCOUNTS_SCHEMA).await;
    for someone in &PEOPLE {
        sqlx::query(
            "INSERT INTO Users (Id, Username, Password, MustUpdatePassword, AudioLanguagePreference,
                 AuthenticationProviderId, PasswordResetProviderId, InvalidLoginAttemptCount, SubtitleMode,
                 SubtitleLanguagePreference, InternalId, RowVersion)
             VALUES (?, ?, '$PBKDF2-SHA512$never read', 0, ?, 'DefaultAuthenticationProvider',
                 'DefaultPasswordResetProvider', 0, 0, ?, ?, 0)",
        )
        .bind(someone.id)
        .bind(someone.name)
        .bind(someone.audio)
        .bind(someone.subtitle)
        .bind(someone.internal_id)
        .execute(&mut connection)
        .await
        .unwrap();
        for (kind, value) in someone.rights {
            sqlx::query("INSERT INTO Permissions (Kind, Value, RowVersion, UserId) VALUES (?, ?, 0, ?)")
                .bind(kind)
                .bind(value)
                .bind(someone.id)
                .execute(&mut connection)
                .await
                .unwrap();
        }
        for (kind, value) in someone.preferences {
            sqlx::query("INSERT INTO Preferences (Kind, Value, RowVersion, UserId) VALUES (?, ?, 0, ?)")
                .bind(kind)
                .bind(value)
                .bind(someone.id)
                .execute(&mut connection)
                .await
                .unwrap();
        }
    }
    connection
}

/// A 10.10 data folder: the accounts in `jellyfin.db`, the items and plays
/// in `library.db`, GUIDs as .NET bytes and the identifiers written together.
async fn ten_point_ten(folder: &Path) {
    let data = folder.join("data");
    std::fs::create_dir_all(&data).unwrap();
    accounts(&data.join("jellyfin.db")).await.close().await.unwrap();
    let mut library = opened(&data.join("library.db")).await;
    run(&mut library, LIBRARY_SCHEMA).await;
    for made in ITEMS.iter().chain(&LISTS) {
        let ids: Vec<String> = made.ids.iter().map(|(provider, id)| format!("{provider}={id}")).collect();
        let data = LISTS.iter().any(|list| list.id == made.id).then(|| list_data(made.id, true));
        sqlx::query(
            "INSERT INTO TypedBaseItems (guid, type, data, ParentId, Path, Name, IndexNumber, ParentIndexNumber,
                 ProductionYear, IsFolder, UserDataKey, SeriesId, ProviderIds, Artists, AlbumArtists, IsVirtualItem)
             VALUES (?, ?, CAST(? AS BLOB), ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, 0)",
        )
        .bind(blob(made.id))
        .bind(made.kind)
        .bind(data)
        .bind(made.parent.map(blob))
        .bind(made.path)
        .bind(made.name)
        .bind(made.index)
        .bind(made.parent_index)
        .bind(made.year)
        .bind(made.user_data_key)
        .bind(made.series.map(blob))
        .bind((!ids.is_empty()).then(|| ids.join("|")))
        .bind(made.artists)
        .bind(made.album_artists)
        .execute(&mut library)
        .await
        .unwrap();
    }
    // One row a key and an account: the copies of the film share theirs.
    for play in &PLAYS {
        let user = PEOPLE.iter().find(|someone| someone.id == play.user).unwrap().internal_id;
        sqlx::query(
            "INSERT OR IGNORE INTO UserDatas (key, userId, rating, played, playCount, isFavorite, playbackPositionTicks,
                 lastPlayedDate, AudioStreamIndex, SubtitleStreamIndex)
             VALUES (?, ?, NULL, ?, ?, ?, ?, ?, NULL, NULL)",
        )
        .bind(play.key)
        .bind(user)
        .bind(play.played)
        .bind(play.count)
        .bind(play.favorite)
        .bind(play.ticks)
        .bind(play.at.map(|at| format!("{at}Z")))
        .execute(&mut library)
        .await
        .unwrap();
    }
    // An account deleted since.
    run(&mut library, "INSERT INTO UserDatas VALUES ('1001', 9, NULL, 1, 1, 1, 0, NULL, NULL, NULL);").await;
    library.close().await.unwrap();
    libraries(&folder.join("root"));
}

/// A 10.11 data folder, or a 12 one with the members of lists in their
/// table: everything in `jellyfin.db`, GUIDs as text, the old library kept
/// beside it and never read.
async fn unified(folder: &Path, twelve: bool) {
    let data = folder.join("data");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("library.db.old"), "not read").unwrap();
    let mut jellyfin = accounts(&data.join("jellyfin.db")).await;
    run(&mut jellyfin, ITEMS_SCHEMA).await;
    sqlx::query("INSERT INTO BaseItems (Id, Type, IsFolder, IsVirtualItem, IsLocked) VALUES (?, 'PLACEHOLDER', 0, 0, 0)")
        .bind(PLACEHOLDER)
        .execute(&mut jellyfin)
        .await
        .unwrap();
    for made in ITEMS.iter().chain(&LISTS) {
        let data = LISTS.iter().any(|list| list.id == made.id).then(|| list_data(made.id, !twelve));
        sqlx::query(
            "INSERT INTO BaseItems (Id, Type, Data, Path, ParentId, SeriesId, Name, IndexNumber, ParentIndexNumber,
                 ProductionYear, Artists, AlbumArtists, IsFolder, IsVirtualItem, IsLocked)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, 0)",
        )
        .bind(made.id)
        .bind(made.kind)
        .bind(data)
        .bind(made.path)
        .bind(made.parent)
        .bind(made.series)
        .bind(made.name)
        .bind(made.index)
        .bind(made.parent_index)
        .bind(made.year)
        .bind(made.artists)
        .bind(made.album_artists)
        .execute(&mut jellyfin)
        .await
        .unwrap();
        for (provider, id) in made.ids {
            sqlx::query("INSERT INTO BaseItemProviders (ItemId, ProviderId, ProviderValue) VALUES (?, ?, ?)")
                .bind(made.id)
                .bind(provider)
                .bind(id)
                .execute(&mut jellyfin)
                .await
                .unwrap();
        }
    }
    for play in &PLAYS {
        sqlx::query(
            "INSERT INTO UserData (ItemId, UserId, CustomDataKey, Rating, PlaybackPositionTicks, PlayCount, IsFavorite,
                 LastPlayedDate, Played, AudioStreamIndex, SubtitleStreamIndex, Likes, RetentionDate)
             VALUES (?, ?, ?, NULL, ?, ?, ?, ?, ?, NULL, NULL, NULL, ?)",
        )
        .bind(play.item)
        .bind(play.user)
        .bind(play.key)
        .bind(play.ticks)
        .bind(play.count)
        .bind(play.favorite)
        .bind(play.at)
        .bind(play.played)
        .bind((play.item == PLACEHOLDER).then_some("2024-06-01 00:00:00"))
        .execute(&mut jellyfin)
        .await
        .unwrap();
    }
    if twelve {
        run(&mut jellyfin, MEMBERS_SCHEMA).await;
        for (parent, order, child, kind) in LINKED {
            sqlx::query("INSERT INTO LinkedChildren (ParentId, SortOrder, ChildId, ChildType) VALUES (?, ?, ?, ?)")
                .bind(parent)
                .bind(order)
                .bind(child)
                .bind(kind)
                .execute(&mut jellyfin)
                .await
                .unwrap();
        }
    }
    jellyfin.close().await.unwrap();
    libraries(&folder.join("root"));
}

/// A GUID as a backup writes it.
fn written(guid: &str) -> String {
    guid.to_lowercase()
}

/// The date as a backup writes it.
fn dated(at: &str) -> String {
    format!("{}Z", at.replace(' ', "T"))
}

fn write(path: &Path, rows: Value) {
    std::fs::write(path, rows.to_string()).unwrap();
}

/// The backup of a 10.11 or a 12 server, unzipped: the same tables written
/// out as JSON, and its libraries.
fn exported(folder: &Path, twelve: bool) {
    let database = folder.join("Database");
    std::fs::create_dir_all(&database).unwrap();
    write(
        &folder.join("manifest.json"),
        json!({
            "ServerVersion": if twelve { "12.4.0" } else { "10.11.11" },
            "BackupEngineVersion": "0.2.0",
            "DateCreated": "2026-01-02T03:04:05.1234567Z",
            "DatabaseTables": ["DbSet`1"],
            "Options": { "Metadata": false, "Trickplay": false, "Subtitles": false, "Database": true },
        }),
    );
    let users: Vec<Value> = PEOPLE
        .iter()
        .map(|someone| {
            json!({
                "Id": written(someone.id),
                "Username": someone.name,
                "NormalizedUsername": someone.name.to_uppercase(),
                "Password": "$PBKDF2-SHA512$never read",
                "MustUpdatePassword": false,
                "AudioLanguagePreference": someone.audio,
                "SubtitleLanguagePreference": someone.subtitle,
                "InternalId": someone.internal_id,
                "Permissions": [],
                "Preferences": [],
                "RowVersion": 0,
            })
        })
        .collect();
    write(&database.join("Users.json"), Value::from(users));
    let mut permissions = Vec::new();
    let mut preferences = Vec::new();
    for someone in &PEOPLE {
        let user = written(someone.id);
        for (kind, value) in someone.rights {
            let id = permissions.len() + 1;
            permissions.push(json!({ "Id": id, "UserId": user, "Kind": kind, "Value": value, "RowVersion": 0 }));
        }
        for (kind, value) in someone.preferences {
            let id = preferences.len() + 1;
            preferences.push(json!({ "Id": id, "UserId": user, "Kind": kind, "Value": value, "RowVersion": 0 }));
        }
    }
    write(&database.join("Permissions.json"), Value::from(permissions));
    write(&database.join("Preferences.json"), Value::from(preferences));
    let mut items =
        vec![json!({ "Id": written(PLACEHOLDER), "Type": "PLACEHOLDER", "IsFolder": false, "IsVirtualItem": false })];
    let mut providers = Vec::new();
    for made in ITEMS.iter().chain(&LISTS) {
        let data = LISTS.iter().any(|list| list.id == made.id).then(|| list_data(made.id, !twelve));
        items.push(json!({
            "Id": written(made.id),
            "Type": made.kind,
            "Data": data,
            "Path": made.path,
            "ParentId": made.parent.map(written),
            "SeriesId": made.series.map(written),
            "Name": made.name,
            "IndexNumber": made.index,
            "ParentIndexNumber": made.parent_index,
            "ProductionYear": made.year,
            "Artists": made.artists,
            "AlbumArtists": made.album_artists,
            "IsFolder": false,
            "IsVirtualItem": false,
            "Provider": null,
            "Children": null,
        }));
        for (provider, id) in made.ids {
            providers.push(
                json!({ "ItemId": written(made.id), "Item": null, "ProviderId": provider, "ProviderValue": id }),
            );
        }
    }
    write(&database.join("BaseItems.json"), Value::from(items));
    write(&database.join("BaseItemProviders.json"), Value::from(providers));
    let plays: Vec<Value> = PLAYS
        .iter()
        .map(|play| {
            json!({
                "CustomDataKey": play.key,
                "Rating": null,
                "PlaybackPositionTicks": play.ticks,
                "PlayCount": play.count,
                "IsFavorite": play.favorite,
                "LastPlayedDate": play.at.map(dated),
                "Played": play.played,
                "AudioStreamIndex": null,
                "SubtitleStreamIndex": null,
                "Likes": null,
                "RetentionDate": (play.item == PLACEHOLDER).then(|| dated("2024-06-01 00:00:00")),
                "ItemId": written(play.item),
                "Item": null,
                "UserId": written(play.user),
                "User": null,
            })
        })
        .collect();
    write(&database.join("UserData.json"), Value::from(plays));
    if twelve {
        let linked: Vec<Value> = LINKED
            .iter()
            .map(|(parent, order, child, kind)| {
                json!({
                    "ParentId": written(parent),
                    "ChildId": written(child),
                    "ChildType": kind,
                    "SortOrder": order,
                    "Parent": null,
                    "Child": null,
                })
            })
            .collect();
        write(&database.join("LinkedChildren.json"), Value::from(linked));
    }
    write(
        &database.join("HistoryRow.json"),
        json!([{ "MigrationId": "20241020103111_LibraryDbMigration", "ProductVersion": "9.0.0" }]),
    );
    libraries(&folder.join("Root"));
}

fn item(guid: &str, kind: Kind, name: &str, path: &str) -> Item {
    Item {
        key: key(guid),
        kind,
        name: name.to_string(),
        year: None,
        path: Some(path.to_string()),
        ids: Vec::new(),
        season: None,
        episode: None,
        parent: None,
        artists: Vec::new(),
    }
}

fn ids(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(provider, id)| (provider.to_string(), id.to_string())).collect()
}

fn assert_read_whole(backup: &Backup) {
    assert_eq!(backup.server, Server::Jellyfin);
    assert_eq!(
        backup.accounts,
        vec![
            Account {
                key: key(ADA),
                name: "Ada".into(),
                administrator: true,
                disabled: false,
                may_delete: true,
                may_download: false,
                libraries: None,
                audio_language: Some("fre".into()),
                subtitle_language: None,
            },
            Account {
                key: key(BEN),
                name: "Ben".into(),
                administrator: false,
                disabled: false,
                may_delete: false,
                may_download: true,
                libraries: Some(vec![key(FILMS)]),
                audio_language: None,
                subtitle_language: Some("eng".into()),
            },
        ]
    );
    assert_eq!(
        backup.libraries,
        vec![Library {
            key: key(FILMS),
            name: "Films".into(),
            folders: vec!["/disks/one/Films".into(), "/disks/two/Films".into()],
        }]
    );
    assert_eq!(
        backup.items,
        vec![
            Item {
                year: Some(2011),
                ids: ids(&[("imdb", "tt0001001"), ("tmdb", "1001")]),
                ..item(HARBOUR, Kind::Movie, "Harbour Lights", "/disks/one/Films/Harbour Lights (2011).mkv")
            },
            Item {
                year: Some(2011),
                ids: ids(&[("tmdb", "1001")]),
                ..item(HARBOUR_4K, Kind::Movie, "Harbour Lights", "/disks/two/Films/Harbour Lights (2011) 4K.mkv")
            },
            Item {
                year: Some(2019),
                ids: ids(&[("tmdb", "3003"), ("tvdb", "2002")]),
                ..item(VALLEY, Kind::Series, "Quiet Valley", "/disks/one/Series/Quiet Valley")
            },
            Item {
                season: Some(2),
                parent: Some(key(VALLEY)),
                ..item(SEASON_TWO, Kind::Season, "Season 2", "/disks/one/Series/Quiet Valley/Season 02")
            },
            Item {
                ids: ids(&[("tvdb", "4004")]),
                season: Some(2),
                episode: Some(5),
                parent: Some(key(VALLEY)),
                ..item(SNOWFALL, Kind::Episode, "Snowfall", SNOWFALL_FILE)
            },
            item(BIRDS, Kind::Artist, "The Paper Birds", "/disks/one/Music/The Paper Birds"),
            Item {
                year: Some(2015),
                ids: ids(&[("musicbrainzalbum", NORTH_WIND_MBID)]),
                artists: vec!["The Paper Birds".into()],
                ..item(NORTH_WIND, Kind::Album, "North Wind", "/disks/one/Music/The Paper Birds/North Wind")
            },
            Item {
                parent: Some(key(NORTH_WIND)),
                artists: vec!["The Paper Birds".into()],
                ..item(FIRST_LIGHT, Kind::Song, "First Light", FIRST_LIGHT_FILE)
            },
            item(PICNIC, Kind::Video, "Picnic", "/disks/one/Home/Picnic.mp4"),
        ]
    );
    let watched = Mark {
        account: key(ADA),
        item: key(HARBOUR),
        played: true,
        play_count: 2,
        position_ms: 0,
        last_played: Some(datetime!(2023-11-14 22:13:20 UTC)),
        favorite: true,
    };
    assert_eq!(
        backup.marks,
        vec![
            watched.clone(),
            Mark { item: key(HARBOUR_4K), ..watched },
            Mark {
                account: key(ADA),
                item: key(SNOWFALL),
                played: false,
                play_count: 0,
                position_ms: 1_234_567,
                last_played: Some(datetime!(2023-11-14 22:15:00.5 UTC)),
                favorite: false,
            },
            Mark {
                account: key(BEN),
                item: key(NORTH_WIND),
                played: false,
                play_count: 0,
                position_ms: 0,
                last_played: None,
                favorite: true,
            },
            Mark {
                account: key(BEN),
                item: key(FIRST_LIGHT),
                played: true,
                play_count: 7,
                position_ms: 0,
                last_played: None,
                favorite: false,
            },
        ]
    );
    assert_eq!(
        backup.playlists,
        vec![Playlist {
            name: "Evening".into(),
            owner: Some(key(ADA)),
            items: vec![key(HARBOUR), key(FIRST_LIGHT), key(PICNIC)],
        }]
    );
    assert_eq!(
        backup.collections,
        vec![Collection { name: "Rainy days".into(), items: vec![key(SNOWFALL)], automatic: None }]
    );
}

#[tokio::test]
async fn a_ten_point_ten_folder_is_read_whole() {
    let folder = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    ten_point_ten(folder.path()).await;
    assert_read_whole(&read(folder.path(), work.path()).await.unwrap());
}

#[tokio::test]
async fn a_ten_point_eleven_folder_is_read_whole() {
    let folder = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    unified(folder.path(), false).await;
    assert_read_whole(&read(folder.path(), work.path()).await.unwrap());
}

#[tokio::test]
async fn a_twelve_folder_is_read_whole() {
    let folder = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    unified(folder.path(), true).await;
    assert_read_whole(&read(folder.path(), work.path()).await.unwrap());
}

#[tokio::test]
async fn an_unzipped_backup_reads_as_the_database_it_was_made_from() {
    for twelve in [false, true] {
        let folder = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        unified(folder.path(), twelve).await;
        exported(backup.path(), twelve);
        let from_folder = read(folder.path(), &work.path().join("folder")).await.unwrap();
        let from_backup = read(backup.path(), &work.path().join("backup")).await.unwrap();
        assert_read_whole(&from_backup);
        assert_eq!(from_backup.server, from_folder.server);
        assert_eq!(from_backup.accounts, from_folder.accounts);
        assert_eq!(from_backup.libraries, from_folder.libraries);
        assert_eq!(from_backup.items, from_folder.items);
        assert_eq!(from_backup.marks, from_folder.marks);
        assert_eq!(from_backup.playlists, from_folder.playlists);
        assert_eq!(from_backup.collections, from_folder.collections);
    }
}

#[tokio::test]
async fn the_data_folder_itself_may_be_given() {
    let folder = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    ten_point_ten(folder.path()).await;
    assert_read_whole(&read(&folder.path().join("data"), work.path()).await.unwrap());
}

#[tokio::test]
async fn a_jellyfin_older_than_ten_point_nine_point_eleven_is_refused() {
    // 10.8: no loudness on the items, no Dolby Vision on the streams, and
    // the accounts' rights still tied to them the old way.
    let accounts_of_ten_point_eight = r#"
        CREATE TABLE "Users" ("Id" TEXT NOT NULL CONSTRAINT "PK_Users" PRIMARY KEY, "Username" TEXT NOT NULL,
            "Password" TEXT NULL, "EasyPassword" TEXT NULL, "InternalId" INTEGER NOT NULL, "RowVersion" INTEGER NOT NULL);
        CREATE TABLE "Permissions" ("Id" INTEGER NOT NULL CONSTRAINT "PK_Permissions" PRIMARY KEY AUTOINCREMENT,
            "Kind" INTEGER NOT NULL, "Value" INTEGER NOT NULL, "RowVersion" INTEGER NOT NULL,
            "Permission_Permissions_Guid" TEXT NULL, "UserId" TEXT NULL);
    "#;
    let libraries = [
        "CREATE TABLE TypedBaseItems (guid GUID primary key NOT NULL, type TEXT NOT NULL, data BLOB NULL,
             ParentId GUID NULL, Path TEXT NULL, UserDataKey Text);
         CREATE TABLE mediastreams (ItemId GUID, StreamIndex INT, StreamType TEXT, PRIMARY KEY (ItemId, StreamIndex));",
        // 10.9 before 10.9.11: the loudness, not yet Dolby Vision.
        "CREATE TABLE TypedBaseItems (guid GUID primary key NOT NULL, type TEXT NOT NULL, data BLOB NULL,
             ParentId GUID NULL, Path TEXT NULL, UserDataKey Text, LUFS Float, NormalizationGain Float);
         CREATE TABLE mediastreams (ItemId GUID, StreamIndex INT, StreamType TEXT, PRIMARY KEY (ItemId, StreamIndex));",
    ];
    for library in libraries {
        let folder = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(folder.path().join("data")).unwrap();
        database(&folder.path().join("data/jellyfin.db"), accounts_of_ten_point_eight).await;
        database(&folder.path().join("data/library.db"), library).await;
        assert!(matches!(
            read(folder.path(), work.path()).await,
            Err(Refusal::TooOld { server: Server::Jellyfin })
        ));
    }
}
