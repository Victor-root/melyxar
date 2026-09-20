//! Rebuilds this crate whenever a migration is added or changed.
//!
//! The migrations are read into the binary at compile time, and nothing in the
//! Rust sources mentions them by name. Without this, adding a migration and
//! compiling changes nothing at all: the build is considered up to date, the
//! binary carries the migrations of the build before it, and the new table or
//! index simply never appears. The failure is silent, it survives a restart,
//! and the only symptom is a server behaving as though the change had never
//! been written.

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
