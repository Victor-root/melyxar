//! An Emby data folder made up here, with the tables and settings files a 4.9
//! server writes, read back as a backup.

use std::path::Path;

use melyxar_import::{Kind, Refusal, Server, read};
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::{Connection, Executor};

const LIBRARY_SCHEMA: &str = "
CREATE TABLE MediaItems (Id INTEGER PRIMARY KEY AUTOINCREMENT, guid GUID, type INT NOT NULL, ParentId INT,
    Path TEXT NULL, IndexNumber INT, Name Text, ParentIndexNumber INT, ProductionYear INT, SeriesId INT,
    AlbumId INT, ProviderIds Text, UserDataKeyId INT);
CREATE TABLE UserDataKeys2 (Id INTEGER PRIMARY KEY AUTOINCREMENT, UserDataKey TEXT NOT NULL COLLATE NOCASE);
CREATE TABLE UserDatas (UserDataKeyId INT not null, userId INT not null, rating float null, played bit not null,
    playCount int not null, isFavorite bit not null, HideFromResume bit not null, playbackPositionTicks bigint not null,
    LastPlayedDateInt INT null, AudioStreamIndex INT, SubtitleStreamIndex INT, RatingLastModified INT not null,
    PlaystateLastModified INT not null, PRIMARY KEY (UserDataKeyId, userId)) WITHOUT ROWID;
CREATE TABLE ItemExtradataTypes (ExtradataTypeId INTEGER PRIMARY KEY AUTOINCREMENT, Name TEXT NOT NULL COLLATE NOCASE);
CREATE TABLE ItemExtradata (ItemId INT NOT NULL, ExtradataTypeId INT NOT NULL, Value TEXT NOT NULL,
    PRIMARY KEY (ItemId, ExtradataTypeId)) WITHOUT ROWID;
CREATE TABLE ItemLinks2 (ItemId INT NOT NULL, Type INT NOT NULL, LinkedId INT NOT NULL, LinkOrder INT NOT NULL,
    PRIMARY KEY (ItemId, Type, LinkedId)) without rowid;
CREATE TABLE ListItems (ListItemEntryId INTEGER PRIMARY KEY, ListId INT NOT NULL, ListItemId INT NOT NULL,
    ListItemOrder INT NOT NULL);
CREATE TABLE UserItemShares (UserId INT NOT NULL, ItemId INT NOT NULL, ShareLevel INT NOT NULL,
    PRIMARY KEY (UserId, ItemId)) WITHOUT ROWID;
CREATE TABLE ImportedCollections (ItemId INT, Name TEXT NOT NULL COLLATE NOCASE, ProviderIds TEXT);
";

/// A library, a film in two copies, a series with an episode, an album with
/// a song by its artist, a playlist, and two collections, one Emby made.
const LIBRARY_ROWS: &str = "
INSERT INTO MediaItems (Id, guid, type, Name) VALUES (3, X'67452301AB89EFCD0123456789ABCDEF', 4, 'Films');
INSERT INTO MediaItems (Id, type, Path) VALUES (4, 3, '/disks/one/Films');
INSERT INTO ItemLinks2 VALUES (3, 8, 4, 0);
INSERT INTO ItemExtradataTypes VALUES (1, 'LibraryOptions');
INSERT INTO ItemExtradata VALUES (3, 1, '{\"PathInfos\":[{\"Path\":\"/disks/one/Films\"},{\"Path\":\"/disks/two/Films\"}]}');
INSERT INTO UserDataKeys2 VALUES (1, 'a'), (2, 'b'), (3, 'c'), (4, 'd'), (5, 'e');
INSERT INTO MediaItems (Id, type, Name, ProductionYear, Path, ProviderIds, UserDataKeyId) VALUES
  (10, 5, 'Harbour Lights', 2011, '/disks/one/Films/Harbour Lights (2011).mkv', 'Tmdb=1001|IMDB=tt0001001|Official Website=https://example.org', 1),
  (11, 5, 'Harbour Lights', 2011, '/disks/two/Films/Harbour Lights (2011) 4K.mkv', 'Tmdb=1001', 1);
INSERT INTO MediaItems (Id, type, Name, ProviderIds) VALUES (20, 6, 'Quiet Valley', 'Tvdb=2002|Tmdb=3003');
INSERT INTO MediaItems (Id, type, Name, Path, IndexNumber, ParentIndexNumber, SeriesId, ProviderIds, UserDataKeyId)
  VALUES (21, 8, 'Snowfall', '/disks/one/Series/Quiet Valley/S02E05.mkv', 5, 2, 20, 'Tvdb=4004', 2);
INSERT INTO MediaItems (Id, type, Name) VALUES (30, 13, 'The Paper Birds'), (31, 13, 'A Guest');
INSERT INTO MediaItems (Id, type, Name, UserDataKeyId) VALUES (32, 12, 'North Wind', 3);
INSERT INTO MediaItems (Id, type, Name, Path, AlbumId, UserDataKeyId)
  VALUES (33, 11, 'First Light', '/disks/one/Music/North Wind/01 First Light.flac', 32, 4);
INSERT INTO ItemLinks2 VALUES (32, 1, 30, 0), (33, 1, 30, 0), (33, 0, 31, 0), (33, 0, 30, 1);
INSERT INTO MediaItems (Id, type, Name) VALUES (40, 16, 'Evening'), (41, 99, 'Rainy days'), (42, 99, 'Harbour Collection');
INSERT INTO MediaItems (Id, type, Name, ProviderIds) VALUES (43, 99, 'Saga', 'Tmdb=777');
INSERT INTO ListItems (ListId, ListItemId, ListItemOrder) VALUES
  (40, 33, 1), (40, 10, 0), (41, 21, 0), (42, 10, 0), (43, 11, 0);
INSERT INTO UserItemShares VALUES (1, 40, 10000), (2, 40, 1);
INSERT INTO ImportedCollections VALUES (42, 'Harbour Collection', 'Tmdb=555');
INSERT INTO UserDatas VALUES
  (1, 1, NULL, 1, 2, 1, 0, 0, 1700000000, NULL, NULL, 0, 0),
  (2, 1, NULL, 0, 0, 0, 0, 12345670000, 1700000100, NULL, NULL, 0, 0),
  (4, 2, NULL, 1, 7, 0, 0, 0, 0, NULL, NULL, 0, 0),
  (3, 2, NULL, 0, 0, 1, 0, 0, NULL, NULL, NULL, 0, 0),
  (5, 1, NULL, 0, 0, 0, 0, 0, NULL, NULL, NULL, 0, 0);
";

const USERS_SCHEMA: &str = "
CREATE TABLE LocalUsersv2 (Id INTEGER PRIMARY KEY AUTOINCREMENT, guid GUID NOT NULL, data BLOB NOT NULL);
INSERT INTO LocalUsersv2 VALUES
  (1, X'00', '{\"UsesIdForConfigurationPath\":true,\"Name\":\"Ada\",\"IdString\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"Password\":\"never read\"}'),
  (2, X'00', '{\"Name\":\"Ben\"}');
";

const POLICY_OF_ADA: &str = r#"<?xml version="1.0"?>
<UserPolicy xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <IsAdministrator>true</IsAdministrator>
  <IsDisabled>false</IsDisabled>
  <EnableContentDeletion>true</EnableContentDeletion>
  <EnableContentDownloading>false</EnableContentDownloading>
  <EnabledFolders />
  <EnableAllFolders>true</EnableAllFolders>
</UserPolicy>"#;

const SETTINGS_OF_ADA: &str = r#"<?xml version="1.0"?>
<UserConfiguration>
  <AudioLanguagePreference>fre</AudioLanguagePreference>
  <SubtitleLanguagePreference></SubtitleLanguagePreference>
</UserConfiguration>"#;

const POLICY_OF_BEN: &str = r#"<?xml version="1.0"?>
<UserPolicy>
  <IsAdministrator>false</IsAdministrator>
  <EnableContentDownloading>true</EnableContentDownloading>
  <EnabledFolders><string>01234567-89ab-cdef-0123-456789abcdef</string></EnabledFolders>
  <EnableAllFolders>false</EnableAllFolders>
</UserPolicy>"#;

async fn database(path: &Path, sql: &str) {
    let options = SqliteConnectOptions::new().filename(path).create_if_missing(true);
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    connection.execute(sqlx::raw_sql(sqlx::AssertSqlSafe(sql))).await.unwrap();
    connection.close().await.unwrap();
}

async fn an_emby_folder(root: &Path) {
    std::fs::create_dir_all(root.join("data")).unwrap();
    database(&root.join("data/library.db"), &format!("{LIBRARY_SCHEMA}{LIBRARY_ROWS}")).await;
    database(&root.join("data/users.db"), USERS_SCHEMA).await;
    let ada = root.join("config/users/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let ben = root.join("config/users/Ben");
    std::fs::create_dir_all(&ada).unwrap();
    std::fs::create_dir_all(&ben).unwrap();
    std::fs::write(ada.join("policy.xml"), POLICY_OF_ADA).unwrap();
    std::fs::write(ada.join("config.xml"), SETTINGS_OF_ADA).unwrap();
    std::fs::write(ben.join("policy.xml"), POLICY_OF_BEN).unwrap();
}

#[tokio::test]
async fn an_emby_folder_is_read_whole() {
    let root = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    an_emby_folder(root.path()).await;
    let backup = read(root.path(), work.path()).await.unwrap();
    assert_eq!(backup.server, Server::Emby);

    let ada = &backup.accounts[0];
    assert_eq!((ada.key.as_str(), ada.name.as_str()), ("1", "Ada"));
    assert!(ada.administrator && ada.may_delete && !ada.may_download && !ada.disabled);
    assert_eq!(ada.libraries, None);
    assert_eq!(ada.audio_language.as_deref(), Some("fre"));
    assert_eq!(ada.subtitle_language, None);
    let ben = &backup.accounts[1];
    assert_eq!(ben.name, "Ben");
    assert!(!ben.administrator && ben.may_download);
    assert_eq!(ben.libraries, Some(vec!["0123456789abcdef0123456789abcdef".to_string()]));

    assert_eq!(backup.libraries.len(), 1);
    assert_eq!(backup.libraries[0].key, "0123456789abcdef0123456789abcdef");
    assert_eq!(backup.libraries[0].folders, vec!["/disks/one/Films", "/disks/two/Films"]);

    let item = |key: &str| backup.items.iter().find(|item| item.key == key).unwrap();
    let film = item("10");
    assert_eq!((film.kind, film.year), (Kind::Movie, Some(2011)));
    assert_eq!(film.ids, vec![("tmdb".to_string(), "1001".to_string()), ("imdb".to_string(), "tt0001001".to_string())]);
    let episode = item("21");
    assert_eq!((episode.kind, episode.season, episode.episode), (Kind::Episode, Some(2), Some(5)));
    assert_eq!(episode.parent.as_deref(), Some("20"));
    assert_eq!(item("20").kind, Kind::Series);
    let song = item("33");
    assert_eq!((song.kind, song.parent.as_deref()), (Kind::Song, Some("32")));
    assert_eq!(song.artists, vec!["The Paper Birds"]);
    assert_eq!(item("32").artists, vec!["The Paper Birds"]);
    assert_eq!(item("30").kind, Kind::Artist);
    assert!(backup.items.iter().all(|item| item.key != "3" && item.key != "40"));

    // The two copies of the film share what Ada did with it.
    let of = |account: &str, key: &str| backup.marks.iter().find(|mark| mark.account == account && mark.item == key);
    for copy in ["10", "11"] {
        let mark = of("1", copy).unwrap();
        assert!(mark.played && mark.favorite);
        assert_eq!(mark.play_count, 2);
        assert_eq!(mark.last_played.unwrap().unix_timestamp(), 1_700_000_000);
    }
    let left = of("1", "21").unwrap();
    assert!(!left.played);
    assert_eq!(left.position_ms, 1_234_567);
    assert_eq!(of("2", "33").unwrap().play_count, 7);
    assert_eq!(of("2", "33").unwrap().last_played, None);
    assert!(of("2", "32").unwrap().favorite);
    assert_eq!(backup.marks.len(), 5);

    assert_eq!(backup.playlists.len(), 1);
    assert_eq!(backup.playlists[0].name, "Evening");
    assert_eq!(backup.playlists[0].owner.as_deref(), Some("1"));
    assert_eq!(backup.playlists[0].items, vec!["10", "33"]);

    let collection = |name: &str| backup.collections.iter().find(|collection| collection.name == name).unwrap();
    assert_eq!(collection("Rainy days").automatic, Some(false));
    assert_eq!(collection("Rainy days").items, vec!["21"]);
    assert_eq!(collection("Harbour Collection").automatic, Some(true));
    assert_eq!(collection("Saga").automatic, Some(true));
}

#[tokio::test]
async fn the_data_folder_itself_may_be_given() {
    let root = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    an_emby_folder(root.path()).await;
    let backup = read(&root.path().join("data"), work.path()).await.unwrap();
    assert_eq!(backup.accounts.len(), 2);
    assert!(backup.accounts[0].administrator);
}

#[tokio::test]
async fn an_emby_older_than_four_point_eight_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("data")).unwrap();
    database(
        &root.path().join("data/library.db"),
        "CREATE TABLE TypedBaseItems (guid GUID PRIMARY KEY, type TEXT, data BLOB);",
    )
    .await;
    database(&root.path().join("data/users.db"), USERS_SCHEMA).await;
    assert!(matches!(
        read(root.path(), work.path()).await,
        Err(Refusal::TooOld { server: Server::Emby })
    ));
}

#[tokio::test]
async fn a_folder_with_nothing_of_a_server_is_said_to_be_so() {
    let root = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    assert!(matches!(read(root.path(), work.path()).await, Err(Refusal::NotAServer)));
    assert!(matches!(
        read(&root.path().join("missing"), work.path()).await,
        Err(Refusal::NotFound(_))
    ));
}
