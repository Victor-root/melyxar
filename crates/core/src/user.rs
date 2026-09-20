//! Accounts, rights and preferences.
//!
//! Even with a single account at first, rights and preferences exist from the
//! first migration: every sensitive feature hangs off them, and adding them
//! later would mean touching every table and every route.

use crate::id::{DeviceId, LibraryId, UserId};
use crate::time::Timestamp;

/// What a person is allowed to do.
///
/// Kept as plain data rather than a role name, so that a right can be granted
/// to one person without inventing a new role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permissions {
    pub is_administrator: bool,
    /// Whether this person sees every library there is, including the ones
    /// added after today.
    ///
    /// Said out loud rather than read from an empty list of grants. Read that
    /// way, an account granted one library whose library was later taken away
    /// would quietly come to see every library on the server, since the grant
    /// goes with the library it names.
    pub sees_every_library: bool,
    /// The libraries this person was granted, read only when they do not see
    /// every one.
    pub allowed_libraries: Vec<LibraryId>,
    /// Highest age rating this person may watch, when one is set.
    pub max_age_rating: Option<i32>,
    pub may_download: bool,
    /// Remove a work from the library.
    pub may_delete: bool,
    /// Also erase the file from disk. Administrators only, and still gated on
    /// the root being writable.
    pub may_delete_from_disk: bool,
    /// Simultaneous playback sessions. None means no limit.
    pub max_sessions: Option<i32>,
}

impl Permissions {
    /// Rights of the default account created at first start.
    pub fn administrator() -> Self {
        Self {
            is_administrator: true,
            sees_every_library: true,
            allowed_libraries: Vec::new(),
            max_age_rating: None,
            may_download: true,
            may_delete: true,
            may_delete_from_disk: true,
            max_sessions: None,
        }
    }

    /// Rights of an ordinary viewer.
    pub fn viewer() -> Self {
        Self {
            is_administrator: false,
            sees_every_library: true,
            allowed_libraries: Vec::new(),
            max_age_rating: None,
            may_download: false,
            may_delete: false,
            may_delete_from_disk: false,
            max_sessions: None,
        }
    }

    /// Whether this person may see the given library.
    pub fn may_access_library(&self, library: LibraryId) -> bool {
        self.sees_every_library || self.allowed_libraries.contains(&library)
    }

    /// Whether anything at all was kept from this person.
    ///
    /// What lets every query that reads the library skip the narrowing
    /// entirely for the ordinary account, which is the one this server mostly
    /// answers.
    pub fn sees_the_whole_server(&self) -> bool {
        self.sees_every_library
    }

    /// Whether this person may see a work carrying the given age rating.
    ///
    /// An unknown rating is allowed: refusing everything unrated would hide
    /// most of a personal library.
    pub fn may_watch_rating(&self, rating: Option<i32>) -> bool {
        match (self.max_age_rating, rating) {
            (None, _) => true,
            (Some(_), None) => true,
            (Some(limit), Some(value)) => value <= limit,
        }
    }
}

/// How multichannel audio is folded down to stereo.
///
/// Browsers output stereo, so this happens on nearly every transcode. Done
/// naively it buries the dialogue under the effects, which is the most common
/// complaint about media servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownmixMethod {
    /// Leave it to the processing tool's own default.
    None,
    /// Splits centre and low frequency into left and right. Keeps the overall
    /// level well, but dialogue stays quiet and bass can dominate.
    CentreAndBassSplit,
    /// Strongly favours the centre channel. Very clear dialogue, quiet
    /// effects. Made for watching late without waking the house.
    NightDialogue,
    /// Spreads the surround channels across both sides while preserving
    /// perceived intensity. Good level, weaker spatial impression.
    IntensityPreserving,
    /// Broadcast standard: measured attenuation on centre and surround, low
    /// frequency channel dropped. The balanced choice and the default.
    #[default]
    BroadcastStandard,
}

impl DownmixMethod {
    /// Every fold this server knows how to perform.
    ///
    /// Listed here rather than in whatever screen offers them, so a method
    /// added to this file appears in the interface instead of quietly existing
    /// where nobody can choose it.
    pub const fn every() -> [Self; 5] {
        [
            Self::None,
            Self::CentreAndBassSplit,
            Self::NightDialogue,
            Self::IntensityPreserving,
            Self::BroadcastStandard,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::CentreAndBassSplit => "centre_and_bass_split",
            Self::NightDialogue => "night_dialogue",
            Self::IntensityPreserving => "intensity_preserving",
            Self::BroadcastStandard => "broadcast_standard",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "centre_and_bass_split" => Some(Self::CentreAndBassSplit),
            "night_dialogue" => Some(Self::NightDialogue),
            "intensity_preserving" => Some(Self::IntensityPreserving),
            "broadcast_standard" => Some(Self::BroadcastStandard),
            _ => None,
        }
    }

    /// Whether asking for this method forces the server to process the audio.
    ///
    /// This is what makes the preference an input of the playback decision:
    /// without it, the setting would silently do nothing on files the browser
    /// can already play, which is worse than having no setting at all.
    pub fn requires_processing(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Lowest and highest gain accepted after a downmix.
pub const MIN_DOWNMIX_GAIN: f64 = 0.5;
pub const MAX_DOWNMIX_GAIN: f64 = 3.0;
pub const DEFAULT_DOWNMIX_GAIN: f64 = 2.0;

/// Which colour scheme the interface uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Light,
    Dark,
    /// Follow the operating system setting.
    #[default]
    System,
}

impl ThemeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "system" => Some(Self::System),
            _ => None,
        }
    }
}

/// Default accent colour: the red picked by the maintainer.
///
/// Used as a fill with white text on it, in both themes. A lighter variant is
/// derived for links and small coloured text on a dark background.
pub const DEFAULT_ACCENT_COLOR: &str = "#c81e1e";

/// Whether this is a colour this server will keep.
///
/// A hash and six hexadecimal digits, and nothing else at all. It is the one
/// value a person chooses that ends up inside a stylesheet, so anything
/// looser would be a way of writing whatever into one: it is checked here
/// rather than wherever it is drawn, because it is drawn in more than one
/// place and the loosest of them would be the one that counts.
pub fn is_an_accent_colour(value: &str) -> bool {
    let Some(digits) = value.strip_prefix('#') else {
        return false;
    };
    digits.len() == 6 && digits.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Whether this is a language an interface can be asked for.
///
/// Two letters, as every language is written in this server. An interface
/// that has no words in it falls back to the ones it has, which is its own to
/// decide; what is kept here is only that the answer is a language at all.
pub fn is_a_language(value: &str) -> bool {
    value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

/// Per person settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Preferences {
    pub interface_language: String,
    /// Preferred audio language, applied when no per-series memory exists.
    pub preferred_audio_language: Option<String>,
    pub preferred_subtitle_language: Option<String>,
    pub theme_mode: ThemeMode,
    pub accent_color: String,
    /// Stylesheet applied to this person only.
    pub custom_css: Option<String>,
    /// Last volume, on the slider scale from zero to one.
    pub volume: f64,
    pub downmix_method: DownmixMethod,
    pub downmix_gain: f64,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            interface_language: "en".to_string(),
            preferred_audio_language: None,
            preferred_subtitle_language: None,
            theme_mode: ThemeMode::default(),
            accent_color: DEFAULT_ACCENT_COLOR.to_string(),
            custom_css: None,
            volume: 1.0,
            downmix_method: DownmixMethod::default(),
            downmix_gain: DEFAULT_DOWNMIX_GAIN,
        }
    }
}

impl Preferences {
    /// Clamps values that came from a client into their accepted range.
    ///
    /// A colour or a language that is neither falls back to the one everybody
    /// starts with rather than failing the whole read: an account that cannot
    /// load locks its owner out, which is far worse than a preference reset.
    pub fn normalised(mut self) -> Self {
        self.volume = self.volume.clamp(0.0, 1.0);
        self.downmix_gain = self.downmix_gain.clamp(MIN_DOWNMIX_GAIN, MAX_DOWNMIX_GAIN);
        if !is_an_accent_colour(&self.accent_color) {
            self.accent_color = DEFAULT_ACCENT_COLOR.to_string();
        }
        if !is_a_language(&self.interface_language) {
            self.interface_language = "en".to_string();
        }
        self
    }
}

/// An account.
#[derive(Debug, Clone, PartialEq)]
pub struct User {
    pub id: UserId,
    pub name: String,
    pub avatar_path: Option<String>,
    pub permissions: Permissions,
    pub preferences: Preferences,
    pub created_at: Timestamp,
}

/// A device holding a long lived token.
///
/// One token per device, individually revocable, so that a television does not
/// have to sign in again every week and a lost device can be cut off without
/// touching the others.
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub id: DeviceId,
    pub user_id: UserId,
    /// Name shown in the device list, for example the browser and system.
    pub name: String,
    pub created_at: Timestamp,
    pub last_seen_at: Timestamp,
}

/// Maps a slider position to the gain actually applied to the audio element.
///
/// Wiring the slider straight to the element is the usual mistake: the ear
/// does not perceive amplitude linearly, so most of the audible change ends up
/// crammed into part of the travel. A moderate power curve spreads the change
/// evenly. The exponent is deliberately gentle, because an aggressive curve is
/// what makes the bottom half of a slider sound like silence.
pub fn volume_curve(slider_position: f64) -> f64 {
    const EXPONENT: f64 = 1.6;
    slider_position.clamp(0.0, 1.0).powf(EXPONENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_survives_a_round_trip_through_its_stored_form() {
        for (mode, written) in [
            (ThemeMode::Light, "light"),
            (ThemeMode::Dark, "dark"),
            (ThemeMode::System, "system"),
        ] {
            assert_eq!(mode.as_str(), written);
            assert_eq!(ThemeMode::parse(written), Some(mode));
        }
        assert_eq!(
            ThemeMode::parse("midnight"),
            None,
            "a theme nobody knows falls back rather than locking the account out"
        );
        assert_eq!(
            ThemeMode::default(),
            ThemeMode::System,
            "without a choice, the theme is the one the machine is set to"
        );
    }

    #[test]
    fn an_ordinary_account_sees_every_library_there_is() {
        let permissions = Permissions::viewer();
        assert!(permissions.sees_the_whole_server());
        assert!(permissions.may_access_library(LibraryId::new()));
    }

    #[test]
    fn an_account_granted_nothing_at_all_sees_nothing_at_all() {
        // The case this is written for: an account granted one library, whose
        // library was later taken away. Read from an empty list of grants it
        // would see everything, and nobody would ever have been told.
        let permissions = Permissions {
            sees_every_library: false,
            allowed_libraries: Vec::new(),
            ..Permissions::viewer()
        };
        assert!(!permissions.sees_the_whole_server());
        assert!(!permissions.may_access_library(LibraryId::new()));
    }

    #[test]
    fn a_restricted_account_only_sees_the_libraries_it_was_granted() {
        let allowed = LibraryId::new();
        let other = LibraryId::new();
        let permissions = Permissions {
            sees_every_library: false,
            allowed_libraries: vec![allowed],
            ..Permissions::viewer()
        };
        assert!(permissions.may_access_library(allowed));
        assert!(!permissions.may_access_library(other));
    }

    #[test]
    fn an_age_limit_only_blocks_works_rated_above_it() {
        let permissions = Permissions {
            max_age_rating: Some(12),
            ..Permissions::viewer()
        };
        assert!(permissions.may_watch_rating(Some(10)));
        assert!(permissions.may_watch_rating(Some(12)));
        assert!(!permissions.may_watch_rating(Some(16)));
    }

    #[test]
    fn an_unrated_work_is_not_hidden_by_an_age_limit() {
        let permissions = Permissions {
            max_age_rating: Some(12),
            ..Permissions::viewer()
        };
        assert!(permissions.may_watch_rating(None));
    }

    #[test]
    fn the_default_downmix_is_the_balanced_broadcast_one() {
        assert_eq!(DownmixMethod::default(), DownmixMethod::BroadcastStandard);
    }

    #[test]
    fn every_downmix_method_except_none_forces_the_server_to_process_audio() {
        assert!(!DownmixMethod::None.requires_processing());
        for method in [
            DownmixMethod::CentreAndBassSplit,
            DownmixMethod::NightDialogue,
            DownmixMethod::IntensityPreserving,
            DownmixMethod::BroadcastStandard,
        ] {
            assert!(
                method.requires_processing(),
                "{method:?} must force processing"
            );
        }
    }

    #[test]
    fn downmix_methods_round_trip_through_their_stored_form() {
        for method in DownmixMethod::every() {
            assert_eq!(DownmixMethod::parse(method.as_str()), Some(method));
        }
    }

    #[test]
    fn the_list_offered_to_a_viewer_holds_every_fold_and_no_repeat() {
        // A method added to the enum and forgotten here would exist in the
        // code and be impossible to choose from a screen.
        let every = DownmixMethod::every();
        let mut names: Vec<&str> = every.iter().map(|method| method.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), every.len(), "each one appears once");
        assert!(every.contains(&DownmixMethod::default()));
    }

    #[test]
    fn preferences_coming_from_a_client_are_clamped_into_range() {
        let wild = Preferences {
            volume: 12.0,
            downmix_gain: 99.0,
            ..Preferences::default()
        }
        .normalised();
        assert_eq!(wild.volume, 1.0);
        assert_eq!(wild.downmix_gain, MAX_DOWNMIX_GAIN);

        let negative = Preferences {
            volume: -3.0,
            downmix_gain: 0.0,
            ..Preferences::default()
        }
        .normalised();
        assert_eq!(negative.volume, 0.0);
        assert_eq!(negative.downmix_gain, MIN_DOWNMIX_GAIN);
    }

    #[test]
    fn a_colour_is_a_hash_and_six_digits_and_nothing_else() {
        assert!(is_an_accent_colour(DEFAULT_ACCENT_COLOR));
        assert!(is_an_accent_colour("#FFFFFF"));
        assert!(is_an_accent_colour("#00ff88"));

        // It ends up inside a stylesheet, so anything looser would be a way
        // of writing whatever into one.
        for nonsense in [
            "",
            "red",
            "c81e1e",
            "#c81e1",
            "#c81e1ee",
            "#ggghhh",
            "#c81e1e; content: url(https://elsewhere)",
            "var(--anything)",
        ] {
            assert!(!is_an_accent_colour(nonsense), "{nonsense} must be refused");
        }
    }

    #[test]
    fn a_preference_that_is_nonsense_falls_back_instead_of_reaching_a_stylesheet() {
        let wild = Preferences {
            accent_color: "#c81e1e; content: url(https://elsewhere)".to_string(),
            interface_language: "not a language".to_string(),
            ..Preferences::default()
        }
        .normalised();
        assert_eq!(wild.accent_color, DEFAULT_ACCENT_COLOR);
        assert_eq!(wild.interface_language, "en");
    }

    #[test]
    fn the_volume_curve_keeps_its_ends_and_rises_without_a_dead_zone() {
        assert_eq!(volume_curve(0.0), 0.0);
        assert_eq!(volume_curve(1.0), 1.0);

        // Half travel must stay clearly audible: an aggressive curve is what
        // makes the lower half of a slider sound like silence.
        let half = volume_curve(0.5);
        assert!(half > 0.25, "half the slider sounded far too quiet: {half}");
        assert!(half < 0.5, "the curve must still compensate for the ear");

        // Strictly increasing across the whole travel.
        let mut previous = 0.0;
        for step in 1..=20 {
            let value = volume_curve(f64::from(step) / 20.0);
            assert!(value > previous, "the curve must never flatten out");
            previous = value;
        }
    }

    #[test]
    fn a_quarter_of_the_slider_is_still_audible() {
        assert!(volume_curve(0.25) > 0.08);
    }
}
