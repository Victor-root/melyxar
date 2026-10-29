//! The backup Jellyfin makes of itself from 10.11, once unzipped: beside its
//! `manifest.json`, every table of its database written out under `Database`
//! as a JSON array, one object a row, named as the columns are. 12 adds
//! `LinkedChildren.json` as its database adds the table.
//!
//! The items of a large library weigh hundreds of megabytes written out this
//! way: each row is kept or let go as soon as it is read, away from the
//! threads that answer requests.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::{DeserializeOwned, SeqAccess, Visitor};

use super::{KINDS, Stored, Tables, key};
use crate::{Refusal, provider};

pub(super) async fn tables(database: PathBuf) -> Result<Tables, Refusal> {
    tokio::task::spawn_blocking(move || read(&database))
        .await
        .map_err(|error| Refusal::Unreadable(error.to_string()))?
}

/// A row of `BaseItemProviders`: one identifier of one item.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Provider {
    #[serde(deserialize_with = "key")]
    item_id: String,
    provider_id: String,
    provider_value: String,
}

fn read(database: &Path) -> Result<Tables, Refusal> {
    let items = table(&database.join("BaseItems.json"), Stored::kept)?;
    let taken: HashSet<&str> =
        items.iter().filter(|item| KINDS.contains(&item.kind.as_str())).map(|item| item.id.as_str()).collect();
    let mut ids: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for row in table(&database.join("BaseItemProviders.json"), |row: Provider| {
        taken.contains(row.item_id.as_str()).then_some(row)
    })? {
        ids.entry(row.item_id).or_default().extend(provider::id(&row.provider_id, &row.provider_value));
    }
    let members = database.join("LinkedChildren.json");
    Ok(Tables {
        users: table(&database.join("Users.json"), Some)?,
        permissions: table(&database.join("Permissions.json"), Some)?,
        preferences: table(&database.join("Preferences.json"), Some)?,
        ids,
        plays: table(&database.join("UserData.json"), Some)?,
        members: if members.is_file() { Some(table(&members, Some)?) } else { None },
        items,
    })
}

/// The rows of a table written out, each kept or let go by `keep` as soon as
/// it is read.
fn table<T: DeserializeOwned, U>(path: &Path, keep: impl FnMut(T) -> Option<U>) -> Result<Vec<U>, Refusal> {
    let unreadable = |error: &dyn fmt::Display| Refusal::Unreadable(format!("{}: {error}", path.display()));
    let file = File::open(path).map_err(|error| unreadable(&error))?;
    let mut deserializer = serde_json::Deserializer::from_reader(BufReader::new(file));
    let rows = serde::Deserializer::deserialize_seq(&mut deserializer, Rows { keep, row: PhantomData })
        .map_err(|error| unreadable(&error))?;
    deserializer.end().map_err(|error| unreadable(&error))?;
    Ok(rows)
}

struct Rows<T, F> {
    keep: F,
    row: PhantomData<fn() -> T>,
}

impl<'de, T: Deserialize<'de>, U, F: FnMut(T) -> Option<U>> Visitor<'de> for Rows<T, F> {
    type Value = Vec<U>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("the rows of a table")
    }

    fn visit_seq<A: SeqAccess<'de>>(mut self, mut rows: A) -> Result<Vec<U>, A::Error> {
        let mut kept = Vec::new();
        while let Some(row) = rows.next_element()? {
            kept.extend((self.keep)(row));
        }
        Ok(kept)
    }
}
