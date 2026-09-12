//! Redaction of media names in logs.
//!
//! Logs are meant to be shared: pasted into a conversation, attached to a bug
//! report, exported as a diagnostic bundle. Media file names say a lot about
//! the person running the server, so by default they never appear in full.
//!
//! A file name is shown as its first four characters followed by an ellipsis,
//! and a path is shown as its root label only. An explicit configuration
//! switch, off by default, restores full names when a scanning problem has to
//! be diagnosed.
//!
//! The switch is a process-wide flag set once at startup, rather than a value
//! threaded through every call site: logging happens everywhere, and a flag
//! carried into every function would be noise with no benefit.

use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Number of leading characters kept when redacting a name.
const KEPT_CHARACTERS: usize = 4;

static REVEAL_MEDIA_NAMES: AtomicBool = AtomicBool::new(false);

/// Sets whether media names appear in full. Called once, at startup, from the
/// loaded configuration.
pub fn set_reveal_media_names(reveal: bool) {
    REVEAL_MEDIA_NAMES.store(reveal, Ordering::Relaxed);
}

/// Whether media names currently appear in full.
pub fn reveal_media_names() -> bool {
    REVEAL_MEDIA_NAMES.load(Ordering::Relaxed)
}

/// Wraps a media file name so that it redacts itself when displayed.
///
/// Use it at every log site that would otherwise print a file name:
/// `tracing::info!(file = %MediaName::new(name), "scanned")`.
pub struct MediaName<'a>(&'a str);

impl<'a> MediaName<'a> {
    pub fn new(name: &'a str) -> Self {
        Self(name)
    }
}

impl fmt::Display for MediaName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if reveal_media_names() {
            return f.write_str(self.0);
        }
        f.write_str(&redact_name(self.0))
    }
}

/// Wraps a media path so that it displays its root label and a redacted file
/// name, never the intermediate folders.
pub struct MediaPath<'a> {
    root_label: &'a str,
    path: &'a Path,
}

impl<'a> MediaPath<'a> {
    pub fn new(root_label: &'a str, path: &'a Path) -> Self {
        Self { root_label, path }
    }
}

impl fmt::Display for MediaPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if reveal_media_names() {
            return write!(f, "{}:{}", self.root_label, self.path.display());
        }
        let name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        write!(f, "{}:.../{}", self.root_label, redact_name(name))
    }
}

/// Keeps the first few characters of a name and replaces the rest.
///
/// Counts characters rather than bytes, so an accented or non-latin name is
/// never cut in the middle of a character.
fn redact_name(name: &str) -> String {
    let mut kept: String = name.chars().take(KEPT_CHARACTERS).collect();
    if name.chars().nth(KEPT_CHARACTERS).is_some() {
        kept.push_str("...");
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The reveal flag is process wide, so tests that touch it run under one
    /// lock and always restore the default.
    static GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_reveal<T>(reveal: bool, body: impl FnOnce() -> T) -> T {
        let _lock = GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        set_reveal_media_names(reveal);
        let outcome = body();
        set_reveal_media_names(false);
        outcome
    }

    #[test]
    fn a_long_name_keeps_only_its_first_characters() {
        with_reveal(false, || {
            assert_eq!(
                MediaName::new("Something.2019.1080p.mkv").to_string(),
                "Some..."
            );
        });
    }

    #[test]
    fn a_short_name_is_not_padded_with_an_ellipsis() {
        with_reveal(false, || {
            assert_eq!(MediaName::new("ab").to_string(), "ab");
            assert_eq!(MediaName::new("abcd").to_string(), "abcd");
        });
    }

    #[test]
    fn redaction_counts_characters_not_bytes() {
        with_reveal(false, || {
            // Five accented characters: cutting on bytes would split one.
            assert_eq!(MediaName::new("ÉtéÀà").to_string(), "ÉtéÀ...");
        });
    }

    #[test]
    fn a_path_shows_its_root_label_and_never_its_folders() {
        with_reveal(false, || {
            let path = PathBuf::from("/somewhere/deep/inside/Something.Long.mkv");
            let shown = MediaPath::new("disk-one", &path).to_string();
            assert_eq!(shown, "disk-one:.../Some...");
            assert!(!shown.contains("deep"));
            assert!(!shown.contains("inside"));
        });
    }

    #[test]
    fn the_explicit_switch_restores_full_names() {
        with_reveal(true, || {
            assert_eq!(
                MediaName::new("Something.2019.mkv").to_string(),
                "Something.2019.mkv"
            );
        });
    }

    // That redaction is the default is checked in its own test binary, in
    // tests/redaction_is_the_default.rs: every test here restores the flag to
    // its default when it is done, so a check made in this process would pass
    // even if the flag started the other way round.
}
