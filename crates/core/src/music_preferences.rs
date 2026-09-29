//! What was chosen for music, by each account and for each library of it,
//! apart from the preferences and options of films: the two share nothing,
//! and neither do their settings (see `docs/architecture/06-musique.md`).

/// What becomes of the music when a film starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilmOnScreen {
    /// Stopped for good, the queue and the bar with it.
    #[default]
    Stop,
    /// Paused, and taken up again where it was once the film is closed.
    Pause,
}

impl FilmOnScreen {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Pause => "pause",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "stop" => Some(Self::Stop),
            "pause" => Some(Self::Pause),
            _ => None,
        }
    }
}

/// How songs are levelled against one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VolumeMode {
    /// Each song as loud as it was made.
    Off,
    /// Every song to the same level.
    #[default]
    Track,
    /// Every album to the same level, its songs keeping the differences
    /// the album was made with: a quiet ballad stays quieter than the song
    /// beside it.
    Album,
}

impl VolumeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Track => "track",
            Self::Album => "album",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "track" => Some(Self::Track),
            "album" => Some(Self::Album),
            _ => None,
        }
    }
}

/// The lowest and highest ceiling a song can be held to, in kilobits a
/// second: below the first nothing is worth hearing, above the second no
/// conversion reaches.
pub const LOWEST_CEILING_KBPS: u32 = 64;
pub const HIGHEST_CEILING_KBPS: u32 = 320;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MusicPreferences {
    pub film_on_screen: FilmOnScreen,
    /// Whether the queue left in a closed tab is there again on the next
    /// visit.
    pub resume_queue: bool,
    /// The most a song may weigh on its way, in kilobits a second. A heavier
    /// one is converted down to it; nothing means every song as it is.
    pub max_bitrate_kbps: Option<u32>,
    pub volume_mode: VolumeMode,
    /// How many seconds one song fades into the next, nought for none.
    pub crossfade_seconds: u32,
}

impl Default for MusicPreferences {
    fn default() -> Self {
        Self {
            film_on_screen: FilmOnScreen::default(),
            resume_queue: true,
            max_bitrate_kbps: None,
            volume_mode: VolumeMode::default(),
            crossfade_seconds: 0,
        }
    }
}

/// What a library of music does beyond what every library does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MusicLibraryOptions {
    /// Whether lyrics found neither in a song nor beside it are asked of
    /// LRCLIB. Off until an administrator turns it on, like every source
    /// online.
    pub lyrics_online: bool,
}

/// The longest a crossfade can be asked to last, in seconds.
pub const LONGEST_CROSSFADE_SECONDS: u32 = 12;

/// A ceiling brought within what can be asked of a conversion.
pub fn bounded_ceiling(kbps: u32) -> u32 {
    kbps.clamp(LOWEST_CEILING_KBPS, HIGHEST_CEILING_KBPS)
}

/// Whether a file of this weight, in bits a second, goes out under the
/// ceiling. A file whose weight was never read goes out as it is: holding it
/// back on a guess would convert songs that never needed it.
pub fn under_the_ceiling(bitrate: Option<i64>, ceiling_kbps: Option<u32>) -> bool {
    match (bitrate, ceiling_kbps) {
        (Some(bits), Some(kbps)) => bits <= i64::from(kbps) * 1000,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_a_film_does_to_the_music_round_trips_through_its_stored_form() {
        for choice in [FilmOnScreen::Stop, FilmOnScreen::Pause] {
            assert_eq!(FilmOnScreen::parse(choice.as_str()), Some(choice));
        }
        assert_eq!(FilmOnScreen::parse("louder"), None);
        for mode in [VolumeMode::Off, VolumeMode::Track, VolumeMode::Album] {
            assert_eq!(VolumeMode::parse(mode.as_str()), Some(mode));
        }
    }

    #[test]
    fn a_ceiling_is_held_between_the_lowest_and_the_highest() {
        assert_eq!(bounded_ceiling(8), LOWEST_CEILING_KBPS);
        assert_eq!(bounded_ceiling(128), 128);
        assert_eq!(bounded_ceiling(9000), HIGHEST_CEILING_KBPS);
    }

    #[test]
    fn a_song_heavier_than_the_ceiling_is_held_back_and_nothing_else() {
        assert!(under_the_ceiling(Some(128_000), Some(128)));
        assert!(!under_the_ceiling(Some(900_000), Some(320)));
        assert!(under_the_ceiling(Some(900_000), None), "no ceiling");
        assert!(under_the_ceiling(None, Some(64)), "a weight never read");
    }
}
