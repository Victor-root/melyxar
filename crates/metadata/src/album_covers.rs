//! The covers of an album offered to be chosen by eye, from MusicBrainz and
//! Deezer, and how one of them is named to be fetched.
//!
//! A cover is named by the service it comes from and a key of that service,
//! never by an address: whoever asks for one cannot make the server fetch
//! anything else, the address being written here from a key that has to
//! look like a key of that service.

/// Where an offered cover comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverSource {
    /// MusicBrainz says which album it is, the Cover Art Archive holds the
    /// picture. Keyed by the album's release group.
    MusicBrainz,
    /// Keyed by the name Deezer gives the picture itself.
    Deezer,
}

impl CoverSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MusicBrainz => "musicbrainz",
            Self::Deezer => "deezer",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "musicbrainz" => Some(Self::MusicBrainz),
            "deezer" => Some(Self::Deezer),
            _ => None,
        }
    }

    /// Whether a key could be one of this service's, and nothing more.
    pub fn takes(self, key: &str) -> bool {
        match self {
            // A release group, as MusicBrainz writes its identifiers.
            Self::MusicBrainz => {
                key.len() == 36
                    && key.char_indices().all(|(at, letter)| match at {
                        8 | 13 | 18 | 23 => letter == '-',
                        _ => letter.is_ascii_hexdigit() && !letter.is_ascii_uppercase(),
                    })
            }
            // The fingerprint of the picture, thirty two hexadecimal letters.
            Self::Deezer => {
                key.len() == 32
                    && key
                        .chars()
                        .all(|letter| letter.is_ascii_hexdigit() && !letter.is_ascii_uppercase())
            }
        }
    }
}

/// How large an offered cover is fetched: small for the grid it is chosen
/// from, large for the one chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverSize {
    Small,
    Large,
}

/// One cover found for an album.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferedCover {
    pub source: CoverSource,
    pub key: String,
    /// The album, as the service names it.
    pub title: String,
    pub artist: Option<String>,
    pub year: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_taken_only_when_it_looks_like_a_key_of_its_service() {
        let release_group = "48117b82-8b0a-3e7c-a2e5-1a0e1b9a2c4d";
        assert!(CoverSource::MusicBrainz.takes(release_group));
        assert!(!CoverSource::MusicBrainz.takes("48117b82-8b0a-3e7c-a2e5-1a0e1b9a2c4"));
        assert!(!CoverSource::MusicBrainz.takes("48117b82/8b0a-3e7c-a2e5-1a0e1b9a2c4d"));
        assert!(!CoverSource::MusicBrainz.takes("../../../../etc/passwd-and-some-more-x"));

        let picture = "5718f7c81c27e0b2417e2a4c45224f8a";
        assert!(CoverSource::Deezer.takes(picture));
        assert!(!CoverSource::Deezer.takes("5718F7C81C27E0B2417E2A4C45224F8A"));
        assert!(!CoverSource::Deezer.takes("5718f7c81c27e0b2417e2a4c45224f8"));
        assert!(!CoverSource::Deezer.takes(release_group));
    }

    #[test]
    fn a_source_reads_back_what_it_writes() {
        for source in [CoverSource::MusicBrainz, CoverSource::Deezer] {
            assert_eq!(CoverSource::parse(source.as_str()), Some(source));
        }
        assert_eq!(CoverSource::parse("tmdb"), None);
    }
}
