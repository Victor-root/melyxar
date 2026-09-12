//! External metadata providers, behind one trait.
//!
//! Everything outside this crate speaks the trait, so the film database can be
//! swapped, doubled or stood in for by a test without anything else changing.
//!
//! Two rules hold here. A provider is asked, never believed blindly: what it
//! returns is data the caller decides about. And a provider that cannot be
//! reached is a delay, never a failure of the library: the scan carries on and
//! the identification is tried again later.

#![forbid(unsafe_code)]

pub mod provider;
pub mod tmdb;

pub use provider::{
    Collection, Credit, MetadataProvider, MovieCandidate, MovieDetails, ProviderError, Trailer,
};
pub use tmdb::TmdbProvider;
