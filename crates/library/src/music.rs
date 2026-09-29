//! Where each song of a folder goes: its album, its artists, its place.
//!
//! A pure reading of what the files say, done a folder at a time because that
//! is the smallest piece of a collection that can answer. Whether an album
//! with no album artist is one artist's or a compilation can only be told by
//! looking at every song on it, and those songs sit together.
//!
//! The rules are those of the servers people come from, so a collection they
//! tagged for one files the same way here (see `docs/architecture/06-musique.md`):
//!
//! * what the file says wins, the folders only filling in what it left out;
//! * an album is its title and whose album it is, so two albums sharing a name
//!   stay two, and one album spread over two disc folders stays one;
//! * an album nobody claimed, whose songs are played by different people, is a
//!   compilation, filed under various artists;
//! * a song keeps its own artists, which are not always the album's: a guest on
//!   one track appears among the artists without the album becoming theirs.

use std::path::Path;

use melyxar_core::music::{AlbumFiling, Named, SongFiling};
use melyxar_tags::Tags;

use crate::naming::sort_title;

/// Whose album a compilation is.
///
/// Written as the servers people come from write it, so the name they know
/// is the name they find.
pub const VARIOUS_ARTISTS: &str = "Various Artists";

/// One song of a folder, with what its file says.
#[derive(Debug, Clone, Copy)]
pub struct FolderSong<'a> {
    pub file_name: &'a str,
    pub tags: &'a Tags,
}

/// Files every song of one folder, in the order they were given.
///
/// `folder` is the folder they sit in, relative to the root of the library:
/// empty for the root itself.
pub fn file_folder(folder: &Path, songs: &[FolderSong<'_>]) -> Vec<SongFiling> {
    let place = Place::of(folder);

    // What each song says of its album, before the folder is asked.
    let albums: Vec<Option<Named>> = songs
        .iter()
        .map(|song| {
            song.tags
                .album
                .as_ref()
                .map(|title| named(title, song.tags.album_sort.as_deref()))
                .or_else(|| place.album.clone())
        })
        .collect();

    // Whose album each of those is, decided once per album for the whole
    // folder: every song of an album has to land on the same one.
    let claims: Vec<Option<Claim>> = albums
        .iter()
        .map(|album| {
            album
                .as_ref()
                .map(|title| claim_for(title, songs, &albums, &place))
        })
        .collect();

    songs
        .iter()
        .zip(albums)
        .zip(claims)
        .map(|((song, album), claim)| {
            let tags = song.tags;
            let (number_in_name, title_in_name) = read_file_name(song.file_name);

            let album = album.zip(claim).map(|(title, claim)| AlbumFiling {
                title,
                artists: claim.artists,
                is_compilation: claim.is_compilation,
            });

            // A song's own artists, and failing those whoever the album is by,
            // unless that is everybody and nobody.
            let artists = match artists_of(&tags.artists, tags.artist_sort.as_deref()) {
                own if !own.is_empty() => own,
                _ => match &album {
                    Some(album) if !album.is_compilation => album.artists.clone(),
                    Some(_) => Vec::new(),
                    None => place.artist.clone().into_iter().collect(),
                },
            };

            SongFiling {
                title: match &tags.title {
                    Some(title) => named(title, tags.title_sort.as_deref()),
                    None => named(&title_in_name, None),
                },
                artists,
                album,
                track: tags.track.or(number_in_name),
                disc: tags.disc.or(place.disc),
                year: tags.year,
                genres: tags.genres.clone(),
            }
        })
        .collect()
}

/// Whose album one album of the folder is.
struct Claim {
    artists: Vec<Named>,
    is_compilation: bool,
}

fn claim_for(
    title: &Named,
    songs: &[FolderSong<'_>],
    albums: &[Option<Named>],
    place: &Place,
) -> Claim {
    let on_it: Vec<&Tags> = songs
        .iter()
        .zip(albums)
        .filter(|(_, album)| {
            album
                .as_ref()
                .is_some_and(|album| album.sort_name == title.sort_name)
        })
        .map(|(song, _)| song.tags)
        .collect();

    // Whoever a song of it names as the album's artist. One is enough: a
    // tagging program that wrote it on one song and not the next meant the
    // same album.
    if let Some(tags) = on_it.iter().find(|tags| !tags.album_artists.is_empty()) {
        let artists = artists_of(&tags.album_artists, tags.album_artist_sort.as_deref());
        let is_compilation = artists
            .iter()
            .any(|artist| artist.sort_name == sort_title(VARIOUS_ARTISTS));
        return Claim {
            artists,
            is_compilation,
        };
    }

    // Nobody claimed it: a compilation when the files say so, or when its
    // songs are by different people.
    let mut players = on_it
        .iter()
        .filter_map(|tags| {
            artists_of(&tags.artists, tags.artist_sort.as_deref())
                .into_iter()
                .next()
        })
        .map(|artist| artist.sort_name);
    let first = players.next();
    let mixed = first
        .as_ref()
        .is_some_and(|first| players.any(|other| &other != first));
    if mixed || on_it.iter().any(|tags| tags.compilation) {
        return Claim {
            artists: vec![named(VARIOUS_ARTISTS, None)],
            is_compilation: true,
        };
    }

    // One artist throughout: theirs. And when no file names anyone, the
    // folders do, if they can.
    let artists = on_it
        .iter()
        .map(|tags| artists_of(&tags.artists, tags.artist_sort.as_deref()))
        .find(|artists| !artists.is_empty())
        .unwrap_or_else(|| place.artist.clone().into_iter().collect());
    Claim {
        artists,
        is_compilation: false,
    }
}

/// What the folders say, for the songs whose files say nothing.
struct Place {
    /// The folder holding the songs, or the one above a disc folder.
    album: Option<Named>,
    /// The folder above the album, and failing that the album folder itself:
    /// a folder of songs sitting straight in the library is usually named
    /// after whoever plays them.
    artist: Option<Named>,
    /// The number of a disc folder such as `CD 2`.
    disc: Option<u32>,
}

impl Place {
    fn of(folder: &Path) -> Self {
        let mut parts: Vec<&str> = folder.iter().filter_map(|part| part.to_str()).collect();
        let disc = parts.last().and_then(|name| disc_of_folder(name));
        if disc.is_some() {
            parts.pop();
        }
        let album = parts.last().map(|name| named(name, None));
        let artist = match parts.len() {
            0 => None,
            1 => album.clone(),
            count => Some(named(parts[count - 2], None)),
        };
        Self {
            album,
            artist,
            disc,
        }
    }
}

/// The folder an album lives in, from the folder one of its songs sits in:
/// the folder above a disc folder, or that folder itself.
pub fn album_folder(folder: &Path) -> &Path {
    let is_disc = folder
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| disc_of_folder(name).is_some());
    match (is_disc, folder.parent()) {
        (true, Some(above)) => above,
        _ => folder,
    }
}

/// The folder above an album's, where the pictures of whoever made it are
/// kept. None for an album sitting straight in the library, whose own folder
/// is usually named after its artist and holds its cover.
pub fn artist_folder(album_folder: &Path) -> Option<&Path> {
    album_folder
        .parent()
        .filter(|above| !above.as_os_str().is_empty())
}

/// The names an album's cover is kept under beside its songs, the most
/// telling first. The small copy a Windows player leaves, `AlbumArtSmall`,
/// is none of them: it is a thumbnail of the cover already there.
const ALBUM_PICTURES: [&str; 5] = ["cover", "folder", "front", "album", "albumart"];

/// The names the picture of an artist is kept under in their folder.
const ARTIST_PICTURES: [&str; 3] = ["artist", "folder", "poster"];

/// The forms a picture beside the songs is read in.
const PICTURE_EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];

/// Whether a file beside the songs is a picture of an album or of an artist,
/// which a walk of a library of music keeps.
pub fn is_music_picture(file_name: &str) -> bool {
    picture_stem(file_name).is_some_and(|stem| {
        ALBUM_PICTURES.contains(&stem.as_str()) || ARTIST_PICTURES.contains(&stem.as_str())
    })
}

/// The cover of an album among the names of the pictures in its folder.
pub fn album_picture<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    best_picture(names, &ALBUM_PICTURES)
}

/// The picture of an artist among the names of the pictures in their folder.
pub fn artist_picture<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    best_picture(names, &ARTIST_PICTURES)
}

fn best_picture<'a>(names: impl IntoIterator<Item = &'a str>, wanted: &[&str]) -> Option<&'a str> {
    names
        .into_iter()
        .filter_map(|name| {
            let stem = picture_stem(name)?;
            let rank = wanted.iter().position(|one| *one == stem)?;
            Some((rank, name))
        })
        .min()
        .map(|(_, name)| name)
}

/// The name of a picture without its extension and its capitals, or nothing
/// for a file that is not a picture.
fn picture_stem(file_name: &str) -> Option<String> {
    let (stem, extension) = file_name.rsplit_once('.')?;
    PICTURE_EXTENSIONS
        .contains(&extension.to_lowercase().as_str())
        .then(|| stem.to_lowercase())
}

/// The number of a folder named after one disc of an album: `CD1`, `Disc 2`,
/// `Disque 3`, `disk_4`.
fn disc_of_folder(name: &str) -> Option<u32> {
    let lowered = name.trim().to_lowercase();
    let rest = ["disque", "disc", "disk", "cd"]
        .iter()
        .find_map(|word| lowered.strip_prefix(word))?;
    let number = rest.trim_start_matches([' ', '_', '-', '.']);
    (!number.is_empty() && number.chars().all(|character| character.is_ascii_digit()))
        .then(|| number.parse().ok())
        .flatten()
        .filter(|number| *number > 0)
}

/// The number a file name starts with, and the rest of it as a title.
///
/// For a file whose tags say nothing: `03 - Quiet Harbour.mp3` is the third
/// song and is called `Quiet Harbour`. A number is only taken when something
/// follows it, so a song called `1979` keeps its name.
fn read_file_name(file_name: &str) -> (Option<u32>, String) {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file_name)
        .trim();
    let digits: String = stem.chars().take_while(char::is_ascii_digit).collect();
    let rest = stem[digits.len()..]
        .trim_start_matches([' ', '.', '-', '_'])
        .trim();
    if digits.is_empty()
        || digits.len() > 3
        || rest.is_empty()
        || rest.len() == stem.len() - digits.len()
    {
        return (None, stem.to_string());
    }
    (
        digits.parse().ok().filter(|number| *number > 0),
        rest.to_string(),
    )
}

/// The artists a field names, the first sorted as the file says when it does.
fn artists_of(names: &[String], first_sort: Option<&str>) -> Vec<Named> {
    names
        .iter()
        .enumerate()
        .map(|(index, name)| named(name, (index == 0).then_some(first_sort).flatten()))
        .collect()
}

fn named(name: &str, sort_as: Option<&str>) -> Named {
    Named {
        name: name.to_string(),
        sort_name: sort_title(sort_as.unwrap_or(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(fill: impl FnOnce(&mut Tags)) -> Tags {
        let mut tags = Tags::default();
        fill(&mut tags);
        tags
    }

    fn names(filed: &[Named]) -> Vec<&str> {
        filed.iter().map(|named| named.name.as_str()).collect()
    }

    fn file(folder: &str, songs: &[(&str, &Tags)]) -> Vec<SongFiling> {
        let songs: Vec<FolderSong<'_>> = songs
            .iter()
            .map(|(file_name, tags)| FolderSong { file_name, tags })
            .collect();
        file_folder(Path::new(folder), &songs)
    }

    #[test]
    fn a_fully_tagged_song_is_filed_by_its_tags_alone() {
        let one = tags(|t| {
            t.title = Some("Quiet Harbour".into());
            t.artists = vec!["Amber Field".into()];
            t.album = Some("Northern Lights".into());
            t.album_artists = vec!["Amber Field".into()];
            t.track = Some(3);
            t.disc = Some(1);
            t.year = Some(2019);
            t.genres = vec!["Folk".into()];
        });
        let filed = file("Whatever/Folder", &[("x.flac", &one)]);
        let song = &filed[0];
        assert_eq!(song.title.name, "Quiet Harbour");
        assert_eq!(names(&song.artists), vec!["Amber Field"]);
        let album = song.album.as_ref().expect("an album");
        assert_eq!(album.title.name, "Northern Lights");
        assert_eq!(names(&album.artists), vec!["Amber Field"]);
        assert!(!album.is_compilation);
        assert_eq!(
            (song.track, song.disc, song.year),
            (Some(3), Some(1), Some(2019))
        );
        assert_eq!(song.genres, vec!["Folk"]);
    }

    #[test]
    fn a_guest_on_one_song_does_not_take_the_album_away_from_its_artist() {
        let first = tags(|t| {
            t.artists = vec!["Amber Field".into()];
            t.album = Some("Northern Lights".into());
            t.album_artists = vec!["Amber Field".into()];
        });
        let with_guest = tags(|t| {
            t.artists = vec!["Amber Field".into(), "The Lanterns".into()];
            t.album = Some("Northern Lights".into());
            t.album_artists = vec!["Amber Field".into()];
        });
        let filed = file(
            "Amber Field/Northern Lights",
            &[("1.mp3", &first), ("2.mp3", &with_guest)],
        );
        assert_eq!(
            names(&filed[1].artists),
            vec!["Amber Field", "The Lanterns"]
        );
        for song in &filed {
            assert_eq!(
                names(&song.album.as_ref().unwrap().artists),
                vec!["Amber Field"]
            );
        }
    }

    #[test]
    fn an_album_nobody_claimed_played_by_different_people_is_a_compilation() {
        let one = tags(|t| {
            t.artists = vec!["Amber Field".into()];
            t.album = Some("Summer Hits".into());
        });
        let two = tags(|t| {
            t.artists = vec!["The Lanterns".into()];
            t.album = Some("Summer Hits".into());
        });
        let filed = file(
            "Compilations/Summer Hits",
            &[("1.mp3", &one), ("2.mp3", &two)],
        );
        for song in &filed {
            let album = song.album.as_ref().unwrap();
            assert!(album.is_compilation);
            assert_eq!(names(&album.artists), vec![VARIOUS_ARTISTS]);
        }
        assert_eq!(
            names(&filed[0].artists),
            vec!["Amber Field"],
            "each song keeps its artist"
        );
        assert_eq!(names(&filed[1].artists), vec!["The Lanterns"]);
    }

    #[test]
    fn a_file_that_says_it_is_on_a_compilation_is_believed() {
        let alone = tags(|t| {
            t.artists = vec!["Amber Field".into()];
            t.album = Some("Summer Hits".into());
            t.compilation = true;
        });
        let album = file("Summer Hits", &[("1.mp3", &alone)])[0]
            .album
            .clone()
            .unwrap();
        assert!(album.is_compilation);
        assert_eq!(names(&album.artists), vec![VARIOUS_ARTISTS]);
    }

    #[test]
    fn an_album_artist_written_on_one_song_holds_for_the_whole_album() {
        let claimed = tags(|t| {
            t.artists = vec!["Amber Field".into()];
            t.album = Some("Northern Lights".into());
            t.album_artists = vec!["Amber Field".into()];
        });
        let unclaimed = tags(|t| {
            t.artists = vec!["The Lanterns".into()];
            t.album = Some("Northern Lights".into());
        });
        let filed = file(
            "Northern Lights",
            &[("1.mp3", &claimed), ("2.mp3", &unclaimed)],
        );
        for song in &filed {
            let album = song.album.as_ref().unwrap();
            assert!(!album.is_compilation, "claimed, so not everybody's");
            assert_eq!(names(&album.artists), vec!["Amber Field"]);
        }
    }

    #[test]
    fn two_albums_in_one_folder_stay_two() {
        let one = tags(|t| {
            t.artists = vec!["Amber Field".into()];
            t.album = Some("Northern Lights".into());
            t.year = Some(2019);
        });
        let other = tags(|t| {
            t.artists = vec!["The Lanterns".into()];
            t.album = Some("Southern Nights".into());
            t.year = Some(2001);
        });
        let filed = file("Mixed", &[("1.mp3", &one), ("2.mp3", &other)]);
        let first = filed[0].album.as_ref().unwrap();
        let second = filed[1].album.as_ref().unwrap();
        assert_eq!(first.title.name, "Northern Lights");
        assert_eq!(names(&first.artists), vec!["Amber Field"]);
        assert_eq!(second.title.name, "Southern Nights");
        assert_eq!(names(&second.artists), vec!["The Lanterns"]);
        assert!(!first.is_compilation && !second.is_compilation);
    }

    #[test]
    fn a_song_that_says_nothing_is_filed_by_its_folders_and_its_name() {
        let nothing = Tags::default();
        let filed = file(
            "Amber Field/Northern Lights",
            &[("03 - Quiet Harbour.mp3", &nothing)],
        );
        let song = &filed[0];
        assert_eq!(song.title.name, "Quiet Harbour");
        assert_eq!(song.track, Some(3));
        let album = song.album.as_ref().unwrap();
        assert_eq!(album.title.name, "Northern Lights");
        assert_eq!(names(&album.artists), vec!["Amber Field"]);
        assert_eq!(names(&song.artists), vec!["Amber Field"]);
    }

    #[test]
    fn a_folder_of_songs_straight_in_the_library_is_the_album_and_its_artist() {
        // Songs dropped in a folder named after whoever plays them.
        let nothing = Tags::default();
        let filed = file(
            "Amber Field",
            &[("Amber Field - Quiet Harbour.mp3", &nothing)],
        );
        let album = filed[0].album.as_ref().unwrap();
        assert_eq!(album.title.name, "Amber Field");
        assert_eq!(names(&album.artists), vec!["Amber Field"]);
        assert_eq!(
            filed[0].title.name, "Amber Field - Quiet Harbour",
            "a name is never split on a guess"
        );
    }

    #[test]
    fn a_song_at_the_top_of_the_library_that_names_no_album_is_on_none() {
        let nothing = Tags::default();
        let filed = file("", &[("Quiet Harbour.mp3", &nothing)]);
        assert_eq!(filed[0].album, None);
        assert!(filed[0].artists.is_empty());
        assert_eq!(filed[0].title.name, "Quiet Harbour");
    }

    #[test]
    fn a_disc_folder_is_one_disc_of_the_album_above_it() {
        let nothing = Tags::default();
        let filed = file(
            "Amber Field/Northern Lights/CD 2",
            &[("01 - Quiet Harbour.flac", &nothing)],
        );
        let song = &filed[0];
        assert_eq!(song.disc, Some(2));
        let album = song.album.as_ref().unwrap();
        assert_eq!(album.title.name, "Northern Lights");
        assert_eq!(names(&album.artists), vec!["Amber Field"]);
    }

    #[test]
    fn the_same_album_in_two_disc_folders_files_as_one() {
        let one = tags(|t| {
            t.album = Some("Northern Lights".into());
            t.artists = vec!["Amber Field".into()];
        });
        let first = file("Amber Field/Northern Lights/Disc 1", &[("1.mp3", &one)]);
        let second = file("Amber Field/Northern Lights/Disc 2", &[("1.mp3", &one)]);
        assert_eq!(first[0].album, second[0].album);
        assert_eq!((first[0].disc, second[0].disc), (Some(1), Some(2)));
    }

    #[test]
    fn a_disc_folder_is_told_from_a_folder_that_merely_starts_like_one() {
        assert_eq!(disc_of_folder("CD1"), Some(1));
        assert_eq!(disc_of_folder("Disc 2"), Some(2));
        assert_eq!(disc_of_folder("disque_3"), Some(3));
        assert_eq!(disc_of_folder("Disk-4"), Some(4));
        assert_eq!(disc_of_folder("Discovery"), None);
        assert_eq!(disc_of_folder("CD"), None);
        assert_eq!(disc_of_folder("Cdiscount 2"), None);
    }

    #[test]
    fn the_folder_of_an_album_is_the_one_above_a_disc_folder() {
        assert_eq!(
            album_folder(Path::new("Amber Field/Northern Lights/CD 2")),
            Path::new("Amber Field/Northern Lights")
        );
        assert_eq!(
            album_folder(Path::new("Amber Field/Northern Lights")),
            Path::new("Amber Field/Northern Lights")
        );
        assert_eq!(
            artist_folder(Path::new("Amber Field/Northern Lights")),
            Some(Path::new("Amber Field"))
        );
        assert_eq!(
            artist_folder(Path::new("Amber Field")),
            None,
            "an album straight in the library"
        );
    }

    #[test]
    fn the_cover_of_an_album_is_the_most_telling_picture_beside_it() {
        let names = ["AlbumArtSmall.jpg", "Folder.jpg", "cover.PNG", "back.jpg"];
        assert_eq!(album_picture(names), Some("cover.PNG"));
        assert_eq!(
            album_picture(["AlbumArtSmall.jpg", "Folder.jpg"]),
            Some("Folder.jpg")
        );
        assert_eq!(
            album_picture(["AlbumArtSmall.jpg", "back.jpg"]),
            None,
            "a thumbnail is not the cover"
        );
        assert_eq!(album_picture(["cover.txt"]), None);
        assert_eq!(
            artist_picture(["folder.jpg", "artist.jpg"]),
            Some("artist.jpg")
        );
    }

    #[test]
    fn a_walk_keeps_only_the_pictures_of_albums_and_artists() {
        for kept in [
            "cover.jpg",
            "Folder.JPG",
            "front.png",
            "artist.webp",
            "poster.jpeg",
        ] {
            assert!(is_music_picture(kept), "{kept}");
        }
        for left in [
            "AlbumArtSmall.jpg",
            "back.jpg",
            "booklet.pdf",
            "cover.txt",
            "scan01.png",
        ] {
            assert!(!is_music_picture(left), "{left}");
        }
    }

    #[test]
    fn a_number_at_the_front_of_a_name_is_a_track_only_when_a_title_follows() {
        assert_eq!(
            read_file_name("03 - Quiet Harbour.mp3"),
            (Some(3), "Quiet Harbour".into())
        );
        assert_eq!(read_file_name("12. Amber.flac"), (Some(12), "Amber".into()));
        assert_eq!(
            read_file_name("7_Lanterns.ogg"),
            (Some(7), "Lanterns".into())
        );
        assert_eq!(read_file_name("1979.mp3"), (None, "1979".into()));
        assert_eq!(
            read_file_name("2046 Nights.mp3"),
            (None, "2046 Nights".into()),
            "four digits are a year"
        );
        assert_eq!(
            read_file_name("Quiet Harbour.mp3"),
            (None, "Quiet Harbour".into())
        );
        assert_eq!(
            read_file_name("99Problems.mp3"),
            (None, "99Problems".into()),
            "part of the name"
        );
    }

    #[test]
    fn a_leading_article_is_left_out_of_how_names_sort_unless_the_file_says_otherwise() {
        let one = tags(|t| {
            t.artists = vec!["The Lanterns".into()];
            t.album = Some("The Long Road".into());
        });
        let filed = file("x", &[("1.mp3", &one)]);
        assert_eq!(filed[0].artists[0].sort_name, "lanterns");
        assert_eq!(
            filed[0].album.as_ref().unwrap().title.sort_name,
            "long road"
        );

        let sorted = tags(|t| {
            t.artists = vec!["Amber Field".into()];
            t.artist_sort = Some("Field, Amber".into());
            t.album = Some("Northern Lights".into());
        });
        let filed = file("x", &[("1.mp3", &sorted)]);
        assert_eq!(filed[0].artists[0].sort_name, "field, amber");
    }
}
