//! Which work of Melyxar each item of the other server is.
//!
//! The other server may have seen the same disks mounted elsewhere, or other
//! copies of the same films, so an item is looked for in turn:
//!
//! 1. by its file: the whole path, then its last two parts, then its name
//!    alone, each only when it points at one work and one item only;
//! 2. by its neighbours: a series is the one its episodes turned out to be in,
//!    an album the one its songs are on, a season the one of that number in
//!    the series;
//! 3. by its identifiers at providers, and an episode by its series, season
//!    and number;
//! 4. by its title: a film with the same title and year, an album with the
//!    same title and artist, a song with the same title on the same album, an
//!    artist with the same name.
//!
//! An item found nowhere stays unmatched: it is listed, never guessed.

use std::collections::HashMap;

use crate::{Item, Kind, Library};

/// One work of Melyxar that an item may turn out to be.
#[derive(Debug, Clone, PartialEq)]
pub struct Ours {
    pub id: String,
    pub kind: Kind,
    pub library: String,
    pub title: String,
    pub year: Option<i32>,
    /// Where its files are, whole.
    pub paths: Vec<String>,
    /// As [`Item::ids`], the provider in lower case.
    pub ids: Vec<(String, String)>,
    /// The number of a season, of an episode within its season, of a song.
    pub number: Option<i32>,
    /// The season of an episode, the series of a season, the album of a song.
    pub parent: Option<String>,
    /// The artists an album or a song is credited to.
    pub artists: Vec<String>,
}

/// How an item was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    File,
    EndOfPath,
    Neighbours,
    Identifiers,
    Title,
}

/// The work each item turned out to be, by the item's key.
pub type Matches = HashMap<String, (String, How)>;

pub fn matched(theirs: &[Item], ours: &[Ours]) -> Matches {
    let mut matches = Matches::new();
    let by_id: HashMap<&str, &Ours> = ours.iter().map(|work| (work.id.as_str(), work)).collect();
    by_files(theirs, ours, &mut matches);
    by_neighbours(theirs, ours, &by_id, &mut matches);
    by_identifiers(theirs, ours, &mut matches);
    by_title(theirs, ours, &mut matches);
    // Found by identifiers and titles, series and albums now point at the
    // seasons and songs below them.
    by_neighbours(theirs, ours, &by_id, &mut matches);
    matches
}

/// The library of Melyxar each library of the other server became: the one
/// most of the items found in its folders are in.
pub fn libraries(libraries: &[Library], theirs: &[Item], ours: &[Ours], matches: &Matches) -> HashMap<String, String> {
    let library_of: HashMap<&str, &str> = ours.iter().map(|work| (work.id.as_str(), work.library.as_str())).collect();
    let mut found = HashMap::new();
    for library in libraries {
        let folders: Vec<String> = library.folders.iter().map(|folder| folded(folder)).collect();
        let mut votes: HashMap<&str, usize> = HashMap::new();
        for item in theirs {
            let Some(path) = item.path.as_deref().map(folded) else {
                continue;
            };
            if !folders.iter().any(|folder| path.starts_with(&format!("{}/", folder.trim_end_matches('/')))) {
                continue;
            }
            if let Some(library) = matches.get(&item.key).and_then(|(work, _)| library_of.get(work.as_str())) {
                *votes.entry(library).or_default() += 1;
            }
        }
        if let Some(most) = most_voted(votes) {
            found.insert(library.key.clone(), most.to_string());
        }
    }
    found
}

/// Whether a work of that kind can be an item of that kind: a film the other
/// server called a video, or the other way round, is the same file.
fn fits(theirs: Kind, ours: Kind) -> bool {
    theirs == ours || matches!((theirs, ours), (Kind::Movie, Kind::Video) | (Kind::Video, Kind::Movie))
}

/// A path written one way whatever system it came from, in lower case.
fn folded(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

/// The last `parts` parts of a folded path.
fn end_of(path: &str, parts: usize) -> Option<String> {
    let all: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    (all.len() >= parts).then(|| all[all.len() - parts..].join("/"))
}

/// A title reduced to its letters and digits, in lower case.
fn plain(title: &str) -> String {
    title
        .chars()
        .map(|letter| if letter.is_alphanumeric() { letter.to_lowercase().next().unwrap_or(letter) } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn most_voted<T: Copy + Ord>(votes: HashMap<T, usize>) -> Option<T> {
    let mut votes: Vec<(T, usize)> = votes.into_iter().collect();
    // Ties go to the smallest, so the same backup gives the same answer.
    votes.sort_by(|one, other| other.1.cmp(&one.1).then(one.0.cmp(&other.0)));
    let (first, count) = *votes.first()?;
    (votes.get(1).is_none_or(|second| second.1 < count)).then_some(first)
}

/// Every value that stands for one entry only, on both sides.
fn unique<'a, K: std::hash::Hash + Eq>(entries: impl Iterator<Item = (K, &'a str)>) -> HashMap<K, &'a str> {
    let mut seen: HashMap<K, Option<&'a str>> = HashMap::new();
    for (key, value) in entries {
        seen.entry(key).and_modify(|kept| {
            if *kept != Some(value) {
                *kept = None;
            }
        }).or_insert(Some(value));
    }
    seen.into_iter().filter_map(|(key, value)| value.map(|value| (key, value))).collect()
}

fn by_files(theirs: &[Item], ours: &[Ours], matches: &mut Matches) {
    let with_files = |kind: Kind| matches!(kind, Kind::Movie | Kind::Video | Kind::Episode | Kind::Song);
    for (how, parts) in [(How::File, None), (How::EndOfPath, Some(2)), (How::EndOfPath, Some(1))] {
        let key_of = |path: &str| -> Option<String> {
            let path = folded(path);
            match parts {
                None => Some(path),
                Some(parts) => end_of(&path, parts),
            }
        };
        let our_paths = unique(ours.iter().filter(|work| with_files(work.kind)).flat_map(|work| {
            work.paths.iter().filter_map(|path| key_of(path)).map(|key| (key, work.id.as_str()))
        }));
        // A path two items share is no more telling than one two works share.
        let their_paths = unique(theirs.iter().filter(|item| with_files(item.kind)).filter_map(|item| {
            item.path.as_deref().and_then(key_of).map(|key| (key, item.key.as_str()))
        }));
        for item in theirs {
            if matches.contains_key(&item.key) || !with_files(item.kind) {
                continue;
            }
            let Some(key) = item.path.as_deref().and_then(key_of) else {
                continue;
            };
            if their_paths.get(&key) != Some(&item.key.as_str()) {
                continue;
            }
            let Some(work) = our_paths.get(&key) else {
                continue;
            };
            if ours.iter().any(|ours| ours.id == *work && fits(item.kind, ours.kind)) {
                matches.insert(item.key.clone(), (work.to_string(), how));
            }
        }
    }
}

fn by_neighbours(theirs: &[Item], ours: &[Ours], by_id: &HashMap<&str, &Ours>, matches: &mut Matches) {
    let parent_of = |id: &str| by_id.get(id).and_then(|work| work.parent.as_deref());
    // A series is where its episodes' seasons are; an album, where its songs are.
    let mut votes: HashMap<&str, HashMap<&str, usize>> = HashMap::new();
    for item in theirs {
        let (Some(parent), Some((work, _))) = (item.parent.as_deref(), matches.get(&item.key)) else {
            continue;
        };
        let above = match item.kind {
            Kind::Episode => parent_of(work).and_then(parent_of),
            Kind::Song => parent_of(work),
            _ => None,
        };
        if let Some(above) = above {
            *votes.entry(parent).or_default().entry(above).or_default() += 1;
        }
    }
    for (parent, votes) in votes {
        if matches.contains_key(parent) {
            continue;
        }
        if let Some(most) = most_voted(votes) {
            matches.insert(parent.to_string(), (most.to_string(), How::Neighbours));
        }
    }
    // A season is the one of its number in its series; an episode, the one of
    // its number in that season.
    for item in theirs {
        if matches.contains_key(&item.key) {
            continue;
        }
        let Some(series) = item.parent.as_deref().and_then(|parent| matches.get(parent)).map(|(work, _)| work.as_str())
        else {
            continue;
        };
        let season_in = |number: i32| {
            ours.iter()
                .find(|work| work.kind == Kind::Season && work.parent.as_deref() == Some(series) && work.number == Some(number))
        };
        let found = match (item.kind, item.season, item.episode) {
            (Kind::Season, Some(season), _) => season_in(season).map(|season| season.id.clone()),
            (Kind::Episode, Some(season), Some(episode)) => season_in(season).and_then(|season| {
                ours.iter()
                    .find(|work| {
                        work.kind == Kind::Episode
                            && work.parent.as_deref() == Some(season.id.as_str())
                            && work.number == Some(episode)
                    })
                    .map(|work| work.id.clone())
            }),
            _ => None,
        };
        if let Some(work) = found {
            matches.insert(item.key.clone(), (work, How::Neighbours));
        }
    }
}

fn by_identifiers(theirs: &[Item], ours: &[Ours], matches: &mut Matches) {
    for kind in [Kind::Movie, Kind::Series, Kind::Album, Kind::Artist] {
        let known = unique(
            ours.iter()
                .filter(|work| fits(kind, work.kind))
                .flat_map(|work| work.ids.iter().map(|(provider, id)| ((provider.as_str(), id.to_lowercase()), work.id.as_str()))),
        );
        for item in theirs.iter().filter(|item| item.kind == kind) {
            if matches.contains_key(&item.key) {
                continue;
            }
            let found = item
                .ids
                .iter()
                .find_map(|(provider, id)| known.get(&(provider.as_str(), id.to_lowercase())));
            if let Some(work) = found {
                matches.insert(item.key.clone(), (work.to_string(), How::Identifiers));
            }
        }
    }
}

fn by_title(theirs: &[Item], ours: &[Ours], matches: &mut Matches) {
    let first_artist = |artists: &[String]| artists.first().map(|artist| plain(artist)).unwrap_or_default();
    let films = unique(
        ours.iter()
            .filter(|work| work.kind == Kind::Movie)
            .filter_map(|work| work.year.map(|year| ((plain(&work.title), year), work.id.as_str()))),
    );
    let albums = unique(
        ours.iter()
            .filter(|work| work.kind == Kind::Album)
            .map(|work| ((plain(&work.title), first_artist(&work.artists)), work.id.as_str())),
    );
    let artists = unique(
        ours.iter().filter(|work| work.kind == Kind::Artist).map(|work| (plain(&work.title), work.id.as_str())),
    );
    for item in theirs {
        if matches.contains_key(&item.key) {
            continue;
        }
        let found = match item.kind {
            Kind::Movie => item.year.and_then(|year| films.get(&(plain(&item.name), year)).copied()),
            Kind::Album => albums.get(&(plain(&item.name), first_artist(&item.artists))).copied(),
            Kind::Artist => artists.get(&plain(&item.name)).copied(),
            Kind::Song => {
                let album = item.parent.as_deref().and_then(|parent| matches.get(parent)).map(|(work, _)| work.as_str());
                album.and_then(|album| {
                    let mut same = ours.iter().filter(|work| {
                        work.kind == Kind::Song && work.parent.as_deref() == Some(album) && plain(&work.title) == plain(&item.name)
                    });
                    match (same.next(), same.next()) {
                        (Some(work), None) => Some(work.id.as_str()),
                        _ => None,
                    }
                })
            }
            _ => None,
        };
        if let Some(work) = found {
            matches.insert(item.key.clone(), (work.to_string(), How::Title));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theirs(key: &str, kind: Kind, name: &str) -> Item {
        Item {
            key: key.into(),
            kind,
            name: name.into(),
            year: None,
            path: None,
            ids: Vec::new(),
            season: None,
            episode: None,
            parent: None,
            artists: Vec::new(),
        }
    }

    fn ours(id: &str, kind: Kind, title: &str) -> Ours {
        Ours {
            id: id.into(),
            kind,
            library: "films".into(),
            title: title.into(),
            year: None,
            paths: Vec::new(),
            ids: Vec::new(),
            number: None,
            parent: None,
            artists: Vec::new(),
        }
    }

    fn found<'a>(matches: &'a Matches, key: &str) -> Option<(&'a str, How)> {
        matches.get(key).map(|(work, how)| (work.as_str(), *how))
    }

    #[test]
    fn a_file_is_found_where_it_is_or_on_a_disk_mounted_elsewhere() {
        let mut same_place = theirs("1", Kind::Movie, "Harbour Lights");
        same_place.path = Some("/disks/one/Films/Harbour Lights.mkv".into());
        let mut elsewhere = theirs("2", Kind::Movie, "Northern Road");
        elsewhere.path = Some(r"D:\Media\Films\Northern Road.mkv".into());
        let mut film = ours("a", Kind::Movie, "Harbour Lights");
        film.paths = vec!["/disks/one/Films/Harbour Lights.mkv".into()];
        let mut other = ours("b", Kind::Video, "Northern Road");
        other.paths = vec!["/mnt/two/films/Northern Road.mkv".into()];
        let matches = matched(&[same_place, elsewhere], &[film, other]);
        assert_eq!(found(&matches, "1"), Some(("a", How::File)));
        assert_eq!(found(&matches, "2"), Some(("b", How::EndOfPath)));
    }

    #[test]
    fn a_name_two_files_share_is_no_proof() {
        let mut one = theirs("1", Kind::Episode, "Pilot");
        one.path = Some("/old/Quiet Valley/S01E01.mkv".into());
        let mut first = ours("a", Kind::Episode, "Pilot");
        first.paths = vec!["/new/Quiet Valley/Season 1/S01E01.mkv".into()];
        let mut second = ours("b", Kind::Episode, "Pilot");
        second.paths = vec!["/new/Other Show/S01E01.mkv".into()];
        assert_eq!(found(&matched(&[one], &[first, second]), "1"), None);
    }

    #[test]
    fn a_series_and_its_seasons_follow_its_episodes_and_the_rest_follow_the_numbers() {
        let series = theirs("s", Kind::Series, "Quiet Valley");
        let mut first = theirs("e1", Kind::Episode, "Pilot");
        first.parent = Some("s".into());
        first.path = Some("/old/Quiet Valley/S01E01.mkv".into());
        (first.season, first.episode) = (Some(1), Some(1));
        let mut later = theirs("e2", Kind::Episode, "Snowfall");
        later.parent = Some("s".into());
        (later.season, later.episode) = (Some(2), Some(5));
        let mut season = theirs("t2", Kind::Season, "Season 2");
        season.parent = Some("s".into());
        season.season = Some(2);

        let our_series = ours("S", Kind::Series, "Quiet Valley");
        let mut one = ours("T1", Kind::Season, "Season 1");
        (one.parent, one.number) = (Some("S".into()), Some(1));
        let mut two = ours("T2", Kind::Season, "Season 2");
        (two.parent, two.number) = (Some("S".into()), Some(2));
        let mut pilot = ours("E1", Kind::Episode, "Pilot");
        (pilot.parent, pilot.number) = (Some("T1".into()), Some(1));
        pilot.paths = vec!["/new/Quiet Valley/Season 1/S01E01.mkv".into()];
        let mut snowfall = ours("E25", Kind::Episode, "Snowfall");
        (snowfall.parent, snowfall.number) = (Some("T2".into()), Some(5));

        let matches = matched(&[series, first, later, season], &[our_series, one, two, pilot, snowfall]);
        assert_eq!(found(&matches, "e1"), Some(("E1", How::EndOfPath)));
        assert_eq!(found(&matches, "s"), Some(("S", How::Neighbours)));
        assert_eq!(found(&matches, "t2"), Some(("T2", How::Neighbours)));
        assert_eq!(found(&matches, "e2"), Some(("E25", How::Neighbours)));
    }

    #[test]
    fn identifiers_then_title_and_year_find_a_film_whose_file_moved() {
        let mut by_ids = theirs("1", Kind::Movie, "Harbour Lights");
        by_ids.ids = vec![("imdb".into(), "TT0001001".into())];
        let mut by_title = theirs("2", Kind::Movie, "Northern Road!");
        by_title.year = Some(2004);
        let mut wrong_year = theirs("3", Kind::Movie, "Northern Road");
        wrong_year.year = Some(1990);
        let mut film = ours("a", Kind::Movie, "Harbour Lights");
        film.ids = vec![("tmdb".into(), "1001".into()), ("imdb".into(), "tt0001001".into())];
        let mut road = ours("b", Kind::Movie, "Northern road");
        road.year = Some(2004);
        let matches = matched(&[by_ids, by_title, wrong_year], &[film, road]);
        assert_eq!(found(&matches, "1"), Some(("a", How::Identifiers)));
        assert_eq!(found(&matches, "2"), Some(("b", How::Title)));
        assert_eq!(found(&matches, "3"), None);
    }

    #[test]
    fn music_follows_its_songs_or_its_names() {
        let mut album = theirs("al", Kind::Album, "North Wind");
        album.artists = vec!["The Paper Birds".into()];
        let artist = theirs("ar", Kind::Artist, "the paper birds");
        let mut song = theirs("so", Kind::Song, "First Light");
        song.parent = Some("al".into());
        let mut other_album = theirs("al2", Kind::Album, "Low Tide");
        other_album.artists = vec!["A Guest".into()];
        let mut moved = theirs("so2", Kind::Song, "Undertow");
        moved.parent = Some("al2".into());
        moved.path = Some("/old/Music/Low Tide/02 Undertow.flac".into());

        let mut our_album = ours("AL", Kind::Album, "North Wind");
        our_album.artists = vec!["The Paper Birds".into()];
        let our_artist = ours("AR", Kind::Artist, "The Paper Birds");
        let mut our_song = ours("SO", Kind::Song, "First Light");
        our_song.parent = Some("AL".into());
        let low_tide = ours("AL2", Kind::Album, "Low Tide (Deluxe)");
        let mut undertow = ours("SO2", Kind::Song, "Undertow");
        undertow.parent = Some("AL2".into());
        undertow.paths = vec!["/music/Low Tide/02 Undertow.flac".into()];

        let matches = matched(&[album, artist, song, other_album, moved], &[our_album, our_artist, our_song, low_tide, undertow]);
        assert_eq!(found(&matches, "al"), Some(("AL", How::Title)));
        assert_eq!(found(&matches, "ar"), Some(("AR", How::Title)));
        assert_eq!(found(&matches, "so"), Some(("SO", How::Title)));
        assert_eq!(found(&matches, "so2"), Some(("SO2", How::EndOfPath)));
        assert_eq!(found(&matches, "al2"), Some(("AL2", How::Neighbours)));
    }

    #[test]
    fn a_library_is_the_one_most_of_its_items_were_found_in() {
        let library = Library { key: "L".into(), name: "Films".into(), folders: vec![r"D:\Media\Films".into()] };
        let mut one = theirs("1", Kind::Movie, "A");
        one.path = Some(r"D:\Media\Films\A.mkv".into());
        let mut two = theirs("2", Kind::Movie, "B");
        two.path = Some(r"D:\Media\Films\B.mkv".into());
        let mut a = ours("a", Kind::Movie, "A");
        a.library = "films".into();
        let mut b = ours("b", Kind::Movie, "B");
        b.library = "films".into();
        let matches: Matches =
            [("1".to_string(), ("a".to_string(), How::File)), ("2".to_string(), ("b".to_string(), How::File))].into();
        assert_eq!(libraries(&[library], &[one, two], &[a, b], &matches).get("L").map(String::as_str), Some("films"));
    }
}
