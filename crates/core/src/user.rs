//! Accounts, rights and preferences.
//!
//! Even with a single account at first, rights and preferences exist from the
//! first migration: every sensitive feature hangs off them, and adding them
//! later would mean touching every table and every route.

use crate::id::{DeviceId, LibraryId, UserId};
use crate::library::LibraryKind;
use crate::time::Timestamp;
use crate::work::ResumeRules;

/// The highest limit of simultaneous streams an account can be given.
pub const MOST_SIMULTANEOUS_STREAMS: i32 = 20;

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

    /// These rights as they are kept, whatever was asked.
    ///
    /// An administrator holds every right and sees every library: the
    /// administration shows the whole server to whoever reaches it, so an
    /// administrator kept from a library would still read it there, and a
    /// limit that is not one is a limit somebody relies on for nothing.
    /// Erasing from the disk is a way of deleting, so it goes with the right
    /// to delete. An account that sees every library holds no list, and a
    /// library granted twice is granted once. A number of simultaneous
    /// streams is kept inside what a household could mean.
    pub fn settled(self) -> Self {
        if self.is_administrator {
            return Self::administrator();
        }
        let mut allowed_libraries = match self.sees_every_library {
            true => Vec::new(),
            false => self.allowed_libraries,
        };
        let mut seen = Vec::with_capacity(allowed_libraries.len());
        allowed_libraries.retain(|library| {
            let first = !seen.contains(library);
            seen.push(*library);
            first
        });
        Self {
            allowed_libraries,
            may_delete_from_disk: self.may_delete && self.may_delete_from_disk,
            max_sessions: self
                .max_sessions
                .map(|most| most.clamp(1, MOST_SIMULTANEOUS_STREAMS)),
            ..self
        }
    }

    /// Whether these rights reach less of the libraries than the ones
    /// before them did: a library that could be read and no longer can.
    pub fn sees_less_than(&self, before: &Self) -> bool {
        if self.sees_every_library {
            return false;
        }
        before.sees_every_library
            || before
                .allowed_libraries
                .iter()
                .any(|library| !self.allowed_libraries.contains(library))
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

/// How tall the banner of the home page is, as a share of the screen's width.
///
/// A share of the width rather than of the height, because what it really sets
/// is how much of the picture behind it survives: those pictures are sixteen
/// by nine, so a third of the width keeps about two thirds of one.
///
/// The range is what is worth looking at. Under a fifth it is a strip that
/// throws away most of every picture, which is the fault this exists to let
/// somebody escape; over half it is a page with nothing under the banner.
///
/// The number everybody starts at is the maintainer's, settled by trying them
/// against his own library rather than against the shelf this was measured
/// on: a little under a third of the width, which keeps about fifty five out
/// of every hundred of a picture and leaves the first row on the screen.
pub const MIN_BANNER_HEIGHT: f64 = 0.20;
pub const MAX_BANNER_HEIGHT: f64 = 0.55;
pub const DEFAULT_BANNER_HEIGHT: f64 = 0.31;

/// Where a band is taken out of a picture taller than the band, from its top.
///
/// Nought keeps the top of the picture and throws away its foot, one does the
/// opposite. A quarter of the way down came out of a shelf of twenty eight
/// real pictures; an eighth came out of the maintainer's own library, which
/// is the larger sample and the one that counts. Both say the same thing,
/// which is that what a banner has to give up is the floor.
pub const DEFAULT_BANNER_CUT: f64 = 0.13;

/// How far the player's step buttons may jump, in seconds.
///
/// Two digits at most, because the length is written inside the button.
pub const SHORTEST_STEP: i64 = 1;
pub const LONGEST_STEP: i64 = 90;
/// What the button back jumps until somebody chooses: a line heard again.
pub const DEFAULT_STEP_BACK: i64 = 10;
/// What the button on jumps until somebody chooses: longer, since what is
/// passed over is a stretch rather than a line.
pub const DEFAULT_STEP_ON: i64 = 30;

/// How far back a film starts from where it was left, at most, in seconds.
/// The same bound as a step, since it is chosen from the same lengths.
pub const LONGEST_REWIND: i64 = LONGEST_STEP;

/// What a viewer wants done with a film of wide gamut colour.
///
/// Wide gamut colour is kept as it is only where the screen shows it: shown
/// on a screen of standard range it looks washed out and grey, so it is
/// converted there. Whether the screen shows it is asked of the browser, and
/// this is the viewer's word over that answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WideGamutChoice {
    /// Kept where the browser says the screen shows it, converted elsewhere.
    #[default]
    Automatic,
    /// Always converted to standard range, for a screen the browser takes for
    /// one that shows it when it does not do it well.
    AlwaysConvert,
    /// Never converted for its colour alone, for a screen that shows it and
    /// that the browser does not say so of.
    NeverConvert,
}

impl WideGamutChoice {
    /// Every choice, in the order a screen offers them.
    pub const fn every() -> [Self; 3] {
        [Self::Automatic, Self::AlwaysConvert, Self::NeverConvert]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::AlwaysConvert => "always_convert",
            Self::NeverConvert => "never_convert",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::every()
            .into_iter()
            .find(|choice| choice.as_str() == value)
    }
}

/// When a film starts with subtitles nobody picked for it.
///
/// Whatever the mode, a subtitle chosen for a film, or turned off in it, is
/// what that film starts with next time. The mode only speaks for a film
/// nobody said anything about.
///
/// "Forced" subtitles are the few lines a film does not say in the language
/// of its soundtrack: a sign, a letter, a scene in another tongue. They are
/// looked for in the language heard first, since that is what they are made
/// for, then in the language read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleMode {
    /// Whole subtitles in the language read, otherwise the forced ones.
    #[default]
    Always,
    /// Whole subtitles in the language read when the film is heard in
    /// another, otherwise the forced ones.
    Smart,
    /// Only ever the forced ones.
    OnlyForced,
    /// The track the file marks as default, otherwise as the smart mode.
    FromTheFile,
    /// Nothing, until asked for in the player.
    Never,
}

impl SubtitleMode {
    /// Every mode, in the order a screen offers them.
    pub const fn every() -> [Self; 5] {
        [
            Self::Always,
            Self::Smart,
            Self::OnlyForced,
            Self::FromTheFile,
            Self::Never,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Smart => "smart",
            Self::OnlyForced => "only_forced",
            Self::FromTheFile => "from_the_file",
            Self::Never => "never",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::every()
            .into_iter()
            .find(|mode| mode.as_str() == value)
    }
}

/// One section of the home page, below its banner.
///
/// The banner is not one of them: it is the picture the page opens on, drawn
/// edge to edge above everything else, and has its own switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeSection {
    /// The tiles leading to each kind of library.
    Band,
    /// What was left halfway.
    CarryOn,
    /// The episode each started series is waiting on.
    UpNext,
    /// The newest works of one kind of library. One section per kind rather
    /// than one for them all, so each row can be moved and hidden on its own.
    Newest(LibraryKind),
    /// Everything newest, whatever its kind.
    RecentlyAdded,
}

impl HomeSection {
    /// Every section, in the order a person meets them until they choose
    /// another: the row of each library, in the order kinds are met, before
    /// the row of everything newest, which mixes them all.
    pub const fn every() -> [Self; 10] {
        [
            Self::Band,
            Self::CarryOn,
            Self::UpNext,
            Self::Newest(LibraryKind::Movies),
            Self::Newest(LibraryKind::Series),
            Self::Newest(LibraryKind::Anime),
            Self::Newest(LibraryKind::HomeMedia),
            Self::Newest(LibraryKind::Shows),
            Self::Newest(LibraryKind::Music),
            Self::RecentlyAdded,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Band => "band",
            Self::CarryOn => "carry_on",
            Self::UpNext => "up_next",
            Self::Newest(LibraryKind::Movies) => "newest:movies",
            Self::Newest(LibraryKind::Series) => "newest:series",
            Self::Newest(LibraryKind::Anime) => "newest:anime",
            Self::Newest(LibraryKind::HomeMedia) => "newest:home_media",
            Self::Newest(LibraryKind::Shows) => "newest:shows",
            Self::Newest(LibraryKind::Music) => "newest:music",
            Self::RecentlyAdded => "recently_added",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::every()
            .into_iter()
            .find(|section| section.as_str() == value)
    }
}

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
    /// When a film starts with subtitles nobody picked for it.
    pub subtitle_mode: SubtitleMode,
    pub theme_mode: ThemeMode,
    pub accent_color: String,
    /// Stylesheet applied to this person only.
    pub custom_css: Option<String>,
    /// Last volume, on the slider scale from zero to one.
    pub volume: f64,
    pub downmix_method: DownmixMethod,
    pub downmix_gain: f64,
    /// How tall the banner of the home page is, as a share of the screen's
    /// width.
    pub banner_height: f64,
    /// Where a band is cut out of a picture taller than the band, between
    /// nought at its top and one at its foot.
    pub banner_cut: f64,
    /// Whether the home page opens on its banner at all. Hidden, the page
    /// starts with its rows, and the other banner settings wait unused.
    pub banner_shown: bool,
    /// Whether the banner draws a fresh handful every time the page is
    /// opened, in place of what was left halfway and what has just arrived.
    pub banner_at_random: bool,
    /// Whether the banner takes the whole window, the height above then
    /// having nothing left to decide.
    pub banner_fills_the_screen: bool,
    /// Whether the bar at the top slides away while a page is read down, and
    /// comes back at the first move up.
    pub header_hides_on_scroll: bool,
    /// Whether this account is left off the list the sign in screen offers.
    ///
    /// The list is a deliberate disclosure, and this is the same choice made
    /// by one person for themselves. Hidden, the account still signs in: the
    /// name is typed rather than pressed.
    pub hidden_at_the_door: bool,
    /// The kinds of library in the order the home page lays them out, its
    /// band and its rows alike. Always every kind, once each.
    pub home_order: Vec<LibraryKind>,
    /// The sections of the home page in the order it lays them out. Always
    /// every section, once each, shown or not.
    pub home_sections: Vec<HomeSection>,
    /// The sections left off the home page. Kept apart from the order, so a
    /// section shown again comes back where it was.
    pub hidden_home_sections: Vec<HomeSection>,
    /// How far the player's button back jumps, in seconds.
    pub step_back_seconds: i64,
    /// How far its button on jumps. Apart from the other, since a line heard
    /// again and a title sequence passed over are not the same length.
    pub step_on_seconds: i64,
    /// How far back a film starts from where it was left, in seconds, so the
    /// last moments seen are seen again. Nought starts it where it was left.
    pub resume_rewind_seconds: i64,
    /// When a work left partway counts as started, as watched, or as too
    /// short to come back to.
    pub resume_rules: ResumeRules,
    /// Whether each kind of library has rules of its own, in place of the
    /// ones above.
    pub resume_rules_per_kind: bool,
    /// The rules of the kinds given some. A kind never given any follows the
    /// rules above until it is.
    pub resume_rules_by_kind: Vec<(LibraryKind, ResumeRules)>,
    /// What is done with a film of wide gamut colour.
    pub wide_gamut: WideGamutChoice,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            interface_language: "en".to_string(),
            preferred_audio_language: None,
            preferred_subtitle_language: None,
            subtitle_mode: SubtitleMode::default(),
            theme_mode: ThemeMode::default(),
            accent_color: DEFAULT_ACCENT_COLOR.to_string(),
            custom_css: None,
            volume: 1.0,
            downmix_method: DownmixMethod::default(),
            downmix_gain: DEFAULT_DOWNMIX_GAIN,
            banner_height: DEFAULT_BANNER_HEIGHT,
            banner_cut: DEFAULT_BANNER_CUT,
            banner_shown: true,
            banner_at_random: false,
            banner_fills_the_screen: false,
            header_hides_on_scroll: true,
            // Shown by default: a household server is the ordinary case, and a
            // list with holes in it is of no use to anybody.
            hidden_at_the_door: false,
            home_order: LibraryKind::every().to_vec(),
            home_sections: HomeSection::every().to_vec(),
            hidden_home_sections: Vec::new(),
            step_back_seconds: DEFAULT_STEP_BACK,
            step_on_seconds: DEFAULT_STEP_ON,
            // Nothing until somebody asks: where a film was left is where it
            // starts, as it always did.
            resume_rewind_seconds: 0,
            resume_rules: ResumeRules::default(),
            resume_rules_per_kind: false,
            resume_rules_by_kind: Vec::new(),
            wide_gamut: WideGamutChoice::default(),
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
        self.banner_height = self
            .banner_height
            .clamp(MIN_BANNER_HEIGHT, MAX_BANNER_HEIGHT);
        self.banner_cut = self.banner_cut.clamp(0.0, 1.0);
        self.step_back_seconds = self.step_back_seconds.clamp(SHORTEST_STEP, LONGEST_STEP);
        self.step_on_seconds = self.step_on_seconds.clamp(SHORTEST_STEP, LONGEST_STEP);
        self.resume_rewind_seconds = self.resume_rewind_seconds.clamp(0, LONGEST_REWIND);
        self.resume_rules = self.resume_rules.normalised();
        // One set per kind, the last one written winning.
        let mut by_kind: Vec<(LibraryKind, ResumeRules)> = Vec::new();
        for (kind, rules) in self.resume_rules_by_kind.into_iter().rev() {
            if !by_kind.iter().any(|(held, _)| *held == kind) {
                by_kind.push((kind, rules.normalised()));
            }
        }
        by_kind.reverse();
        self.resume_rules_by_kind = by_kind;
        if !is_an_accent_colour(&self.accent_color) {
            self.accent_color = DEFAULT_ACCENT_COLOR.to_string();
        }
        if !is_a_language(&self.interface_language) {
            self.interface_language = "en".to_string();
        }
        // A kind named twice keeps its first place, and one never named goes
        // after the others, in the order everybody starts with: a kind added
        // to the server must not be missing from anybody's home page.
        let mut order: Vec<LibraryKind> = Vec::new();
        for kind in self.home_order.into_iter().chain(LibraryKind::every()) {
            if !order.contains(&kind) {
                order.push(kind);
            }
        }
        self.home_order = order;
        // The same for the sections, for the same reason.
        let mut sections: Vec<HomeSection> = Vec::new();
        for section in self.home_sections.into_iter().chain(HomeSection::every()) {
            if !sections.contains(&section) {
                sections.push(section);
            }
        }
        self.home_sections = sections;
        let mut hidden: Vec<HomeSection> = Vec::new();
        for section in self.hidden_home_sections {
            if !hidden.contains(&section) {
                hidden.push(section);
            }
        }
        self.hidden_home_sections = hidden;
        self
    }

    /// The rules a work of this kind of library is held to.
    pub fn resume_rules_for(&self, kind: LibraryKind) -> ResumeRules {
        match self.resume_rules_per_kind {
            true => self
                .resume_rules_by_kind
                .iter()
                .find(|(held, _)| *held == kind)
                .map_or(self.resume_rules, |(_, rules)| *rules),
            false => self.resume_rules,
        }
    }

    /// The sections the home page shows, in the order it shows them.
    pub fn home_sections_shown(&self) -> Vec<HomeSection> {
        self.home_sections
            .iter()
            .copied()
            .filter(|section| !self.hidden_home_sections.contains(section))
            .collect()
    }

    /// Where a kind of library comes on this person's home page, first at
    /// nought.
    pub fn place_on_the_home_page(&self, kind: LibraryKind) -> usize {
        self.home_order
            .iter()
            .position(|chosen| *chosen == kind)
            .unwrap_or(self.home_order.len())
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
    fn an_administrator_is_kept_with_every_right_whatever_was_asked() {
        let asked = Permissions {
            is_administrator: true,
            sees_every_library: false,
            allowed_libraries: vec![LibraryId::new()],
            may_delete: false,
            max_sessions: Some(1),
            ..Permissions::viewer()
        };
        assert_eq!(asked.settled(), Permissions::administrator());
    }

    #[test]
    fn rights_are_kept_consistent_with_one_another() {
        let library = LibraryId::new();
        let settled = Permissions {
            sees_every_library: false,
            allowed_libraries: vec![library, library],
            may_delete: false,
            may_delete_from_disk: true,
            max_sessions: Some(500),
            ..Permissions::viewer()
        }
        .settled();
        assert_eq!(settled.allowed_libraries, vec![library], "granted once");
        assert!(
            !settled.may_delete_from_disk,
            "erasing from the disk is a way of deleting"
        );
        assert_eq!(settled.max_sessions, Some(MOST_SIMULTANEOUS_STREAMS));

        let everything = Permissions {
            sees_every_library: true,
            allowed_libraries: vec![library],
            max_sessions: Some(0),
            ..Permissions::viewer()
        }
        .settled();
        assert!(everything.allowed_libraries.is_empty());
        assert_eq!(everything.max_sessions, Some(1));
        assert_eq!(Permissions::viewer().settled(), Permissions::viewer());
    }

    #[test]
    fn rights_that_reach_fewer_libraries_say_so() {
        let films = LibraryId::new();
        let series = LibraryId::new();
        let only = |granted: Vec<LibraryId>| Permissions {
            sees_every_library: false,
            allowed_libraries: granted,
            ..Permissions::viewer()
        };
        let every = Permissions::viewer();

        assert!(only(vec![films]).sees_less_than(&every));
        assert!(only(vec![films]).sees_less_than(&only(vec![films, series])));
        assert!(!only(vec![films, series]).sees_less_than(&only(vec![films])));
        assert!(!every.sees_less_than(&only(vec![films])));
        assert!(!only(vec![films]).sees_less_than(&only(vec![films])));
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
    fn subtitle_modes_round_trip_through_their_stored_form() {
        for mode in SubtitleMode::every() {
            assert_eq!(SubtitleMode::parse(mode.as_str()), Some(mode));
        }
        assert_eq!(SubtitleMode::parse("sometimes"), None);
        assert_eq!(SubtitleMode::default(), SubtitleMode::Always);
    }

    #[test]
    fn wide_gamut_choices_round_trip_through_their_stored_form() {
        for choice in WideGamutChoice::every() {
            assert_eq!(WideGamutChoice::parse(choice.as_str()), Some(choice));
        }
        assert_eq!(WideGamutChoice::parse("sometimes"), None);
        assert_eq!(WideGamutChoice::default(), WideGamutChoice::Automatic);
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
    fn a_banner_is_never_a_hairline_nor_the_whole_page() {
        let squashed = Preferences {
            banner_height: 0.01,
            banner_cut: -2.0,
            ..Preferences::default()
        }
        .normalised();
        assert_eq!(squashed.banner_height, MIN_BANNER_HEIGHT);
        assert_eq!(squashed.banner_cut, 0.0);

        let swollen = Preferences {
            banner_height: 9.0,
            banner_cut: 4.0,
            ..Preferences::default()
        }
        .normalised();
        assert_eq!(swollen.banner_height, MAX_BANNER_HEIGHT);
        assert_eq!(swollen.banner_cut, 1.0);

        // And what nobody touched comes back untouched.
        let usual = Preferences::default().normalised();
        assert_eq!(usual.banner_height, DEFAULT_BANNER_HEIGHT);
        assert_eq!(usual.banner_cut, DEFAULT_BANNER_CUT);
    }

    #[test]
    fn a_kind_follows_the_rules_of_every_kind_until_it_is_given_its_own() {
        let films = ResumeRules {
            min_percent: 2,
            max_percent: 95,
            min_seconds: 600,
        };
        let mut chosen = Preferences {
            resume_rules_by_kind: vec![
                (LibraryKind::Movies, ResumeRules::default()),
                (LibraryKind::Movies, films),
            ],
            ..Preferences::default()
        }
        .normalised();
        assert_eq!(
            chosen.resume_rules_by_kind,
            vec![(LibraryKind::Movies, films)],
            "one set per kind, the last written"
        );
        assert_eq!(
            chosen.resume_rules_for(LibraryKind::Movies),
            ResumeRules::default(),
            "rules of their own count only once asked for"
        );

        chosen.resume_rules_per_kind = true;
        assert_eq!(chosen.resume_rules_for(LibraryKind::Movies), films);
        assert_eq!(
            chosen.resume_rules_for(LibraryKind::Anime),
            chosen.resume_rules,
            "a kind never given rules follows those of every kind"
        );
    }

    #[test]
    fn the_home_sections_always_hold_every_section_once() {
        let anime = HomeSection::Newest(LibraryKind::Anime);
        let chosen = Preferences {
            home_sections: vec![anime, HomeSection::Band, anime],
            hidden_home_sections: vec![HomeSection::UpNext, HomeSection::UpNext],
            ..Preferences::default()
        }
        .normalised();
        assert_eq!(chosen.home_sections.len(), HomeSection::every().len());
        assert_eq!(
            chosen.home_sections[..4],
            [anime, HomeSection::Band, HomeSection::CarryOn, HomeSection::UpNext]
        );
        assert_eq!(chosen.hidden_home_sections, vec![HomeSection::UpNext]);
        assert!(
            !chosen.home_sections_shown().contains(&HomeSection::UpNext),
            "a hidden section is left out of what is shown"
        );
        assert_eq!(
            chosen.home_sections_shown().len(),
            HomeSection::every().len() - 1,
            "and keeps its place in the order"
        );
        assert_eq!(
            Preferences::default().home_sections.last(),
            Some(&HomeSection::RecentlyAdded),
            "the row of every library before the row that mixes them all"
        );
        for section in HomeSection::every() {
            assert_eq!(HomeSection::parse(section.as_str()), Some(section));
        }
        assert_eq!(HomeSection::parse("libraries"), None);
    }

    #[test]
    fn the_home_order_always_holds_every_kind_once() {
        let chosen = Preferences {
            home_order: vec![LibraryKind::Anime, LibraryKind::Shows, LibraryKind::Anime],
            ..Preferences::default()
        }
        .normalised();
        assert_eq!(
            chosen.home_order,
            vec![
                LibraryKind::Anime,
                LibraryKind::Shows,
                LibraryKind::Movies,
                LibraryKind::Series,
                LibraryKind::HomeMedia,
                LibraryKind::Music,
            ]
        );
        assert_eq!(chosen.place_on_the_home_page(LibraryKind::Anime), 0);
        assert_eq!(chosen.place_on_the_home_page(LibraryKind::Movies), 2);

        assert_eq!(
            Preferences::default().normalised().home_order,
            LibraryKind::every().to_vec()
        );
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
