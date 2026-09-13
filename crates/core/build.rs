//! Writes the build into the binary, so a running server can say which one it
//! is.
//!
//! Without this, a version number changes only when somebody remembers to
//! change it, and every report of a problem starts with the same question: is
//! this the build that has the fix in it. The commit answers it.
//!
//! Nothing here may fail the build. A copy of the source with no history, or a
//! machine with no git, still compiles and says so.

use std::process::Command;

fn main() {
    println!("cargo:rustc-env=MELYXAR_BUILD={}", build_name());

    // Without these the answer is computed once and then frozen into the
    // cached build for ever, which is the one thing this must not do.
    for path in [
        ".git/HEAD",
        ".git/refs/heads/develop",
        ".git/refs/heads/main",
    ] {
        println!("cargo:rerun-if-changed=../../{path}");
    }
}

/// The short commit, plus a mark when the source has been edited since.
fn build_name() -> String {
    let Some(commit) = git(&["rev-parse", "--short=8", "HEAD"]) else {
        return "no history".to_string();
    };
    // An empty answer means nothing differs from the commit.
    match git(&["status", "--porcelain"]).is_some_and(|changes| !changes.is_empty()) {
        true => format!("{commit}+edited"),
        false => commit,
    }
}

fn git(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}
