//! How much of a library one run goes over.
//!
//! A scan is asked for by somebody with a reason, and the reasons are not the
//! same one. Most of the time the question is what turned up on the disk since
//! last time, and reading four hundred films again to answer it would be an
//! evening spent on nothing. Sometimes a film has a name and no picture, and
//! what is wanted is the holes filled. And once in a while something was read
//! wrong from end to end, and the only answer is to do the lot again.
//!
//! Three modes rather than a switch per pass: these are the three questions
//! anybody actually asks, they are the three the other servers offer, and a
//! screen can word them in a sentence each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshMode {
    /// What the disk holds that was not here last time, and nothing else.
    ///
    /// The cheapest of the three, and the one a scan nobody asked for uses: a
    /// file is opened only when its size or its date moved, and a work that
    /// already carries a name is left exactly as it is.
    NewAndUpdatedFiles,
    /// The above, plus another go at whatever is still missing.
    ///
    /// The usual one, and what this server did before there was a choice at
    /// all. A picture is asked for the day a film is named and never again, so
    /// one that did not arrive that day would never arrive: a disk that was
    /// full for a minute, a provider that was down. This is the pass that goes
    /// back for it.
    #[default]
    WhatIsMissing,
    /// Everything again, whatever is already recorded.
    ///
    /// Every file read again for what it says of itself, and every work put
    /// back in the queue to be asked about. The heaviest by far, so it is
    /// never what a scan does on its own: somebody asks for it, on one
    /// library, knowing what it costs.
    ///
    /// A film somebody named by hand is left alone even here. That is what
    /// picking by hand means, and a correction undone by a button nobody
    /// connected to it is the worst kind of surprise.
    Everything,
}

impl RefreshMode {
    /// Every mode there is.
    ///
    /// Written down rather than left to whoever needs the list: a screen needs
    /// a sentence for each, and one with none reaches it as its own name.
    pub const ALL: [Self; 3] = [
        Self::NewAndUpdatedFiles,
        Self::WhatIsMissing,
        Self::Everything,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::NewAndUpdatedFiles => "new_and_updated_files",
            Self::WhatIsMissing => "what_is_missing",
            Self::Everything => "everything",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "new_and_updated_files" => Some(Self::NewAndUpdatedFiles),
            "what_is_missing" => Some(Self::WhatIsMissing),
            "everything" => Some(Self::Everything),
            _ => None,
        }
    }

    /// Whether every file is read again rather than only the ones nobody has
    /// read yet.
    pub fn reads_every_file_again(self) -> bool {
        matches!(self, Self::Everything)
    }

    /// Whether works that already carry a name are asked about again.
    pub fn asks_about_named_works_again(self) -> bool {
        matches!(self, Self::Everything)
    }

    /// Whether the holes of a film that has a name are gone back for.
    ///
    /// True of the two heavier modes: the lightest one exists precisely to
    /// touch nothing but what is new.
    pub fn fills_in_what_is_missing(self) -> bool {
        matches!(self, Self::WhatIsMissing | Self::Everything)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_survives_a_round_trip_through_its_stored_form() {
        for mode in RefreshMode::ALL {
            assert_eq!(RefreshMode::parse(mode.as_str()), Some(mode));
        }
        assert_eq!(RefreshMode::parse("something_new"), None);
    }

    #[test]
    fn a_run_nobody_chose_a_mode_for_is_the_one_this_server_always_did() {
        // Filling in the holes of a named film is a decision of this project,
        // not a mode among three: a default that skipped it would quietly undo
        // it, and the only sign would be a grey rectangle somebody scrolls
        // past months later.
        assert_eq!(RefreshMode::default(), RefreshMode::WhatIsMissing);
        assert!(RefreshMode::default().fills_in_what_is_missing());
    }

    #[test]
    fn the_lightest_mode_touches_nothing_that_is_already_recorded() {
        let light = RefreshMode::NewAndUpdatedFiles;
        assert!(!light.reads_every_file_again());
        assert!(!light.asks_about_named_works_again());
        assert!(!light.fills_in_what_is_missing());
    }

    #[test]
    fn the_heaviest_mode_does_every_part_of_the_lighter_ones() {
        let everything = RefreshMode::Everything;
        assert!(everything.reads_every_file_again());
        assert!(everything.asks_about_named_works_again());
        assert!(
            everything.fills_in_what_is_missing(),
            "a run that does the lot cannot leave a hole a lighter run would have filled"
        );
    }
}
