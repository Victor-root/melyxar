//! Internal identifiers.
//!
//! Every entity owns a server-generated UUID version 7. Version 7 embeds a
//! timestamp, so identifiers sort by creation order, which keeps database
//! indexes compact. External identifiers such as the ones from metadata
//! providers are stored separately and never used as primary keys.

use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

/// Declares a newtype over [`Uuid`] so that identifiers of different entities
/// cannot be mixed up by accident.
macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generates a new time-ordered identifier.
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Wraps an existing UUID, typically one read from the database.
            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            /// Hyphenated lowercase form, the one stored and sent over the API.
            pub fn to_db_string(&self) -> String {
                self.0.as_hyphenated().to_string()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0.as_hyphenated(), f)
            }
        }

        impl FromStr for $name {
            type Err = crate::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value)
                    .map(Self)
                    .map_err(|_| crate::Error::invalid_input(concat!("malformed ", stringify!($name))))
            }
        }
    };
}

define_id!(
    /// Identifies a person holding an account on this server.
    UserId
);
define_id!(
    /// Identifies a library, which groups one or more root folders.
    LibraryId
);
define_id!(
    /// Identifies a root folder inside a library.
    LibraryRootId
);
define_id!(
    /// Identifies a work: the thing a viewer picks, never a file on disk.
    WorkId
);
define_id!(
    /// Identifies one physical file backing a work.
    MediaSourceId
);
define_id!(
    /// Identifies a single stream inside a media source.
    TrackId
);
define_id!(
    /// Identifies one chapter of a media source.
    ChapterId
);
define_id!(
    /// Identifies a video attached to a work without being the work itself,
    /// such as a trailer.
    ExtraVideoId
);
define_id!(
    /// Identifies a person credited on a work.
    PersonId
);
define_id!(
    /// Identifies a collection, either provider-supplied or hand made.
    CollectionId
);
define_id!(
    /// Identifies a device holding a long lived access token.
    DeviceId
);
define_id!(
    /// Identifies a background job.
    JobId
);
define_id!(
    /// Identifies a live playback session held in memory by the server.
    SessionId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_round_trip_through_their_text_form() {
        let id = WorkId::new();
        let parsed: WorkId = id.to_string().parse().expect("valid identifier");
        assert_eq!(id, parsed);
    }

    #[test]
    fn malformed_identifiers_are_rejected() {
        assert!("not-a-uuid".parse::<WorkId>().is_err());
    }

    #[test]
    fn version_seven_identifiers_sort_by_creation_order() {
        let first = WorkId::new();
        let second = WorkId::new();
        assert!(first <= second, "v7 identifiers must be time ordered");
    }
}
