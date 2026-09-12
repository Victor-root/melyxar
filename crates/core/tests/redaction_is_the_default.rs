//! A log that has not been told otherwise censors media names.
//!
//! This lives on its own, in its own test binary, and must stay that way.
//! Whether names are revealed is a flag for the whole process, and every test
//! that turns it on puts it back afterwards, so a check made alongside them
//! would pass whatever the flag started as. Here nothing has touched it, so
//! what is read is what a server that was never configured would do.

use melyxar_core::privacy::{reveal_media_names, MediaName, MediaPath};
use std::path::PathBuf;

#[test]
fn a_server_nobody_configured_censors_the_names_it_logs() {
    assert!(
        !reveal_media_names(),
        "a log is meant to be pasted into a conversation: names are censored \
         unless someone turned that off on purpose"
    );

    assert_eq!(
        MediaName::new("Something.2019.1080p.mkv").to_string(),
        "Some..."
    );

    let path = PathBuf::from("/somewhere/deep/inside/Something.Long.mkv");
    let shown = MediaPath::new("disk-one", &path).to_string();
    assert_eq!(shown, "disk-one:.../Some...");
    assert!(!shown.contains("deep"), "a path never shows its folders");
}
