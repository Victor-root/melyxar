//! Short, stable names for generated files.
//!
//! An image served under a name that changes when its content changes can be
//! cached for ever by a browser, which is the whole point: a poster fetched
//! once is never fetched again, and a poster replaced is fetched again at once
//! without anyone clearing anything.
//!
//! This is a name, not a seal. It says two files differ, not that nobody
//! tampered with one.

/// Builds the name a piece of content is served under.
pub fn of_bytes(content: &[u8]) -> String {
    format!("{:016x}", fnv1a(content))
}

/// Builds the name from a string, such as the path a provider gave.
///
/// A provider that changes a picture changes its path, so the path already
/// carries the change and hashing the bytes would cost a second pass over
/// them for nothing.
pub fn of_text(value: &str) -> String {
    format!("{:016x}", fnv1a(value.as_bytes()))
}

/// The hash itself, chosen for being short, fast and easy to read back in a
/// file name rather than for being hard to forge.
fn fnv1a(content: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    content.iter().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_content_always_gets_the_same_name() {
        assert_eq!(of_bytes(b"a poster"), of_bytes(b"a poster"));
        assert_eq!(of_text("/abc123.jpg"), of_text("/abc123.jpg"));
    }

    #[test]
    fn a_path_a_provider_gave_is_named_the_same_way_as_its_bytes_would_be() {
        assert_eq!(of_text("/abc123.jpg"), of_bytes(b"/abc123.jpg"));
        assert_ne!(
            of_text("/abc123.jpg"),
            of_text("/def456.jpg"),
            "a provider that changed the picture changed its path"
        );
    }

    #[test]
    fn content_that_changed_gets_another_name() {
        assert_ne!(of_bytes(b"a poster"), of_bytes(b"another poster"));
        assert_ne!(
            of_bytes(b"a poster"),
            of_bytes(b"a poster "),
            "a name that survives a change would be served stale for ever"
        );
    }

    #[test]
    fn a_name_is_short_and_safe_to_put_in_a_path() {
        let name = of_bytes(b"a poster");
        assert_eq!(name.len(), 16);
        assert!(name.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn nothing_still_has_a_name() {
        assert_eq!(of_bytes(&[]).len(), 16);
    }
}
